use super::{
    DivergenceKind, ObserverDivergence, ObserverError, ObserverReplayConfig, ObserverReplayStatus,
};
use crate::da::{DaError, DataAvailability};
use crate::signer::BatchCommitment;
use crate::storage::ObserverCheckpoint;
use l2_core::address::is_l2_zero_address;
use l2_core::crypto::{decode_public_key, derive_account_id, hash_domain, verify_signature};
use l2_core::{
    canonical_batch_data_hash, decode_batch_data, merkle_root, withdrawal_merkle_root, Account,
    AccountType, BatchDataDecodeError, DeterministicExecutor, ExecutionConfig, Hash32,
    L2TransactionKind, Receipt, SignedL2Transaction, State, WithdrawalLeaf,
};

pub(super) struct ReplayOutcome {
    pub(super) status: ObserverReplayStatus,
    pub(super) next_checkpoint: Option<ObserverCheckpoint>,
    pub(super) divergence: Option<ObserverDivergence>,
}

impl ReplayOutcome {
    fn diverged(status: ObserverReplayStatus, divergence: ObserverDivergence) -> Self {
        Self {
            status,
            next_checkpoint: None,
            divergence: Some(divergence),
        }
    }
}

pub(super) async fn replay_commitment(
    config: &ObserverReplayConfig,
    da: &dyn DataAvailability,
    checkpoint: &ObserverCheckpoint,
    commitment: &BatchCommitment,
) -> Result<ReplayOutcome, ObserverError> {
    let payload = match da
        .read_batch_payload_by_hash(commitment.block_height, commitment.roots_b.data_hash)
        .await
    {
        Ok(Some(payload)) => payload,
        Ok(None) | Err(DaError::Unavailable) => {
            return Ok(ReplayOutcome::diverged(
                ObserverReplayStatus::MissingDa,
                divergence(
                    commitment,
                    DivergenceKind::MissingDa,
                    "batch data unavailable",
                ),
            ));
        }
        Err(error @ DaError::HashMismatch { .. })
        | Err(error @ DaError::BlockHashMismatch { .. })
        | Err(error @ DaError::InvalidPublicReference)
        | Err(error @ DaError::AmbiguousPublicPayload)
        | Err(error @ DaError::PayloadTooLarge { .. }) => {
            return Ok(ReplayOutcome::diverged(
                ObserverReplayStatus::CorruptDa,
                divergence(commitment, DivergenceKind::CorruptDa, error.safe_reason()),
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let actual_data_hash = hash_domain("l2.batch.data.v1", &[&payload.payload_bytes]);
    if actual_data_hash != commitment.roots_b.data_hash {
        return Ok(ReplayOutcome::diverged(
            ObserverReplayStatus::CorruptDa,
            root_divergence(
                commitment,
                DivergenceKind::CorruptDa,
                "data_hash",
                commitment.roots_b.data_hash,
                actual_data_hash,
            ),
        ));
    }
    let decoded = match decode_batch_data(&payload.payload_bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            return Ok(ReplayOutcome::diverged(
                ObserverReplayStatus::CorruptDa,
                divergence(commitment, DivergenceKind::CorruptDa, error.safe_reason()),
            ));
        }
    };
    if decoded.transactions.len() != decoded.receipts.len() {
        return Ok(ReplayOutcome::diverged(
            ObserverReplayStatus::CorruptDa,
            divergence(
                commitment,
                DivergenceKind::CorruptDa,
                "transaction and receipt counts differ",
            ),
        ));
    }
    if decoded.transactions.len() > config.max_txs_per_block {
        return Ok(ReplayOutcome::diverged(
            ObserverReplayStatus::Invalid,
            divergence(
                commitment,
                DivergenceKind::RootMismatch,
                "transaction count exceeds configured block limit",
            ),
        ));
    }

    let mut state = checkpoint.state.clone();
    let replay = replay_transactions(
        config,
        commitment.block_height,
        &mut state,
        &decoded.transactions,
    );
    if let Some((tx_index, expected, actual)) =
        first_receipt_mismatch(&decoded.receipts, &replay.receipts)
    {
        return Ok(ReplayOutcome::diverged(
            ObserverReplayStatus::Invalid,
            receipt_divergence(commitment, tx_index, expected, actual),
        ));
    }

    let roots = ReplayedRoots::from_replay(
        &decoded.transactions,
        &replay.receipts,
        &replay.withdrawals,
        &state,
    )?;
    if let Some(divergence) = roots.first_mismatch(commitment) {
        return Ok(ReplayOutcome::diverged(
            ObserverReplayStatus::Invalid,
            divergence,
        ));
    }
    let next_checkpoint = ObserverCheckpoint {
        next_batch_no: commitment
            .batch_no
            .checked_add(1)
            .ok_or(ObserverError::InvalidRequest("batch number overflow"))?,
        next_block_height: commitment
            .block_height
            .checked_add(1)
            .ok_or(ObserverError::InvalidRequest("block height overflow"))?,
        state_root: roots.state_root,
        state,
    };
    Ok(ReplayOutcome {
        status: ObserverReplayStatus::Valid,
        next_checkpoint: Some(next_checkpoint),
        divergence: None,
    })
}

pub(super) fn validate_commitment_order(
    checkpoint: &ObserverCheckpoint,
    commitment: &BatchCommitment,
) -> Option<ObserverDivergence> {
    if commitment.batch_no != checkpoint.next_batch_no {
        return Some(divergence(
            commitment,
            DivergenceKind::NonContiguousCommitment,
            "batch number is not contiguous with checkpoint",
        ));
    }
    if commitment.block_height != checkpoint.next_block_height {
        return Some(divergence(
            commitment,
            DivergenceKind::NonContiguousCommitment,
            "block height is not contiguous with checkpoint",
        ));
    }
    if commitment.roots_a.prev_state_root != checkpoint.state_root {
        return Some(root_divergence(
            commitment,
            DivergenceKind::RootMismatch,
            "prev_state_root",
            checkpoint.state_root,
            commitment.roots_a.prev_state_root,
        ));
    }
    None
}

struct ReplayBatch {
    receipts: Vec<Receipt>,
    withdrawals: Vec<WithdrawalLeaf>,
}

fn replay_transactions(
    config: &ObserverReplayConfig,
    block_height: u64,
    state: &mut State,
    txs: &[SignedL2Transaction],
) -> ReplayBatch {
    let executor = DeterministicExecutor;
    let mut receipts = Vec::with_capacity(txs.len());
    let mut withdrawals = Vec::new();
    let mut block_gas_used = 0u64;

    for tx in txs {
        if let Err(reason) = verify_tx(state, tx, config) {
            receipts.push(Receipt::rejected(tx.tx_hash(), reason));
            continue;
        }
        let required_gas = config.gas_schedule.required_gas(&tx.kind);
        let Some(next_block_gas_used) = block_gas_used.checked_add(required_gas) else {
            receipts.push(Receipt::rejected(tx.tx_hash(), "block_gas_limit_exceeded"));
            continue;
        };
        if next_block_gas_used > config.block_gas_limit {
            receipts.push(Receipt::rejected(tx.tx_hash(), "block_gas_limit_exceeded"));
            continue;
        }
        let outcome = executor.apply(
            state,
            tx,
            &ExecutionConfig {
                block_time: 0,
                block_height,
                gas_coin_asset: config.gas_coin_asset,
                gas_schedule: config.gas_schedule,
                max_internal_messages: config.max_internal_messages,
                ..ExecutionConfig::default()
            },
        );
        block_gas_used = next_block_gas_used;
        receipts.push(outcome.receipt);
        withdrawals.extend(outcome.withdrawals);
    }

    ReplayBatch {
        receipts,
        withdrawals,
    }
}

fn verify_tx(
    state: &State,
    tx: &SignedL2Transaction,
    config: &ObserverReplayConfig,
) -> Result<(), &'static str> {
    if tx.chain_id != config.chain_id {
        return Err("wrong_chain_id");
    }
    if is_canonical_system_deposit(tx) {
        return validate_reserved_zero_addresses(tx, true);
    }
    if tx.is_system() {
        return Err("deposit_must_be_system");
    }
    let from = tx.from.ok_or("missing_sender")?;
    if is_l2_zero_address(from) {
        return Err("reserved_zero_address");
    }
    let public_key_hex = tx.public_key.as_deref().ok_or("missing_public_key")?;
    let signature_hex = tx.signature.as_deref().ok_or("missing_signature")?;
    let account = state.account(from).ok_or("unknown_sender")?;
    validate_public_sender_account(account)?;
    let public_key = decode_public_key(public_key_hex).map_err(|_| "invalid_public_key")?;
    validate_account_public_key(from, account, &public_key)?;
    if !verify_signature(public_key_hex, signature_hex, &tx.signing_payload()) {
        return Err("bad_signature");
    }
    if account.nonce != tx.nonce {
        return Err("bad_nonce");
    }
    validate_reserved_zero_addresses(tx, false)
}

fn validate_public_sender_account(account: &Account) -> Result<(), &'static str> {
    if account.flags.disabled {
        return Err("account_disabled");
    }
    if account.is_recovery_locked() {
        return Err("account_recovery_locked");
    }
    if account.flags.system_only || matches!(account.account_type, AccountType::System) {
        return Err("sender_system_only");
    }
    if account.flags.contract_only || matches!(account.account_type, AccountType::Contract) {
        return Err("sender_contract_only");
    }
    if !account.can_send_public_transaction() {
        return Err("sender_not_public");
    }
    Ok(())
}

fn validate_account_public_key(
    from: Hash32,
    account: &Account,
    public_key: &[u8; 32],
) -> Result<(), &'static str> {
    if let Some(active_public_key) = account.active_public_key {
        if active_public_key.as_bytes() == public_key {
            return Ok(());
        }
        return Err("public_key_sender_mismatch");
    }
    if derive_account_id(public_key) != from {
        return Err("public_key_sender_mismatch");
    }
    Ok(())
}

fn is_canonical_system_deposit(tx: &SignedL2Transaction) -> bool {
    matches!(tx.kind, L2TransactionKind::Deposit { .. })
        && tx.from.is_none()
        && tx.public_key.is_none()
        && tx.signature.is_none()
}

fn validate_reserved_zero_addresses(
    tx: &SignedL2Transaction,
    allow_system_deposit: bool,
) -> Result<(), &'static str> {
    match tx.kind {
        L2TransactionKind::Deposit { recipient, .. } if is_l2_zero_address(recipient) => {
            Err("reserved_zero_address")
        }
        L2TransactionKind::Deposit { .. } if allow_system_deposit => Ok(()),
        L2TransactionKind::Deposit { .. } => Err("deposit_must_be_system"),
        L2TransactionKind::Transfer { to, .. } if is_l2_zero_address(to) => {
            Err("reserved_zero_address")
        }
        L2TransactionKind::DeployContract { contract, .. }
        | L2TransactionKind::CallContract { contract, .. }
            if is_l2_zero_address(contract) =>
        {
            Err("reserved_zero_address")
        }
        _ => Ok(()),
    }
}

struct ReplayedRoots {
    tx_root: Hash32,
    receipt_root: Hash32,
    withdrawal_root: Hash32,
    state_root: Hash32,
    data_hash: Hash32,
}

impl ReplayedRoots {
    fn from_replay(
        txs: &[SignedL2Transaction],
        receipts: &[Receipt],
        withdrawals: &[WithdrawalLeaf],
        state: &State,
    ) -> Result<Self, ObserverError> {
        Ok(Self {
            tx_root: merkle_root(
                &txs.iter()
                    .map(SignedL2Transaction::tx_hash)
                    .collect::<Vec<_>>(),
            ),
            receipt_root: merkle_root(&receipts.iter().map(Receipt::leaf_hash).collect::<Vec<_>>()),
            withdrawal_root: withdrawal_merkle_root(withdrawals)?,
            state_root: state.root_hash(),
            data_hash: canonical_batch_data_hash(txs, receipts),
        })
    }

    fn first_mismatch(&self, commitment: &BatchCommitment) -> Option<ObserverDivergence> {
        compare_root(
            commitment,
            "tx_root",
            commitment.roots_a.tx_root,
            self.tx_root,
        )
        .or_else(|| {
            compare_root(
                commitment,
                "receipt_root",
                commitment.roots_b.receipt_root,
                self.receipt_root,
            )
        })
        .or_else(|| {
            compare_root(
                commitment,
                "withdrawal_root",
                commitment.roots_b.withdrawal_root,
                self.withdrawal_root,
            )
        })
        .or_else(|| {
            compare_root(
                commitment,
                "state_root",
                commitment.roots_a.state_root,
                self.state_root,
            )
        })
        .or_else(|| {
            compare_root(
                commitment,
                "data_hash",
                commitment.roots_b.data_hash,
                self.data_hash,
            )
        })
    }
}

fn first_receipt_mismatch<'a>(
    expected: &'a [Receipt],
    actual: &'a [Receipt],
) -> Option<(usize, &'a Receipt, &'a Receipt)> {
    expected
        .iter()
        .zip(actual)
        .enumerate()
        .find_map(|(index, (expected, actual))| {
            (expected != actual).then_some((index, expected, actual))
        })
}

fn compare_root(
    commitment: &BatchCommitment,
    field: &'static str,
    expected: Hash32,
    actual: Hash32,
) -> Option<ObserverDivergence> {
    (expected != actual).then(|| {
        root_divergence(
            commitment,
            DivergenceKind::RootMismatch,
            field,
            expected,
            actual,
        )
    })
}

fn divergence(
    commitment: &BatchCommitment,
    kind: DivergenceKind,
    reason: &'static str,
) -> ObserverDivergence {
    ObserverDivergence {
        batch_no: commitment.batch_no,
        block_height: commitment.block_height,
        kind,
        field: None,
        tx_index: None,
        expected_hash: None,
        actual_hash: None,
        reason,
    }
}

fn root_divergence(
    commitment: &BatchCommitment,
    kind: DivergenceKind,
    field: &'static str,
    expected: Hash32,
    actual: Hash32,
) -> ObserverDivergence {
    ObserverDivergence {
        field: Some(field),
        expected_hash: Some(expected),
        actual_hash: Some(actual),
        ..divergence(commitment, kind, "root mismatch")
    }
}

fn receipt_divergence(
    commitment: &BatchCommitment,
    tx_index: usize,
    expected: &Receipt,
    actual: &Receipt,
) -> ObserverDivergence {
    ObserverDivergence {
        tx_index: Some(tx_index),
        expected_hash: Some(expected.leaf_hash()),
        actual_hash: Some(actual.leaf_hash()),
        ..divergence(
            commitment,
            DivergenceKind::ReceiptMismatch,
            "receipt mismatch",
        )
    }
}

trait SafeReason {
    fn safe_reason(&self) -> &'static str;
}

impl SafeReason for DaError {
    fn safe_reason(&self) -> &'static str {
        match self {
            DaError::Unavailable => "batch data unavailable",
            DaError::PayloadTooLarge { .. } => "batch data oversized",
            DaError::HashMismatch { .. } => "batch data hash mismatch",
            DaError::BlockHashMismatch { .. } => "batch block hash mismatch",
            DaError::InvalidPublicReference => "batch public reference invalid",
            DaError::AmbiguousPublicPayload => "batch public reference ambiguous",
            DaError::PublicIo(_) => "public DA filesystem failed",
            DaError::Storage(_) => "storage failed",
        }
    }
}

impl SafeReason for BatchDataDecodeError {
    fn safe_reason(&self) -> &'static str {
        match self {
            BatchDataDecodeError::InvalidMagic => "batch data magic invalid",
            BatchDataDecodeError::UnsupportedVersion => "batch data version unsupported",
            BatchDataDecodeError::WrongType => "batch data type invalid",
            BatchDataDecodeError::UnexpectedEof => "batch data ended unexpectedly",
            BatchDataDecodeError::TrailingBytes => "batch data has trailing bytes",
            BatchDataDecodeError::LengthOverflow => "batch data length overflow",
            BatchDataDecodeError::InvalidOption => "batch data option invalid",
            BatchDataDecodeError::InvalidTag => "batch data tag invalid",
            BatchDataDecodeError::InvalidUtf8 => "batch data utf8 invalid",
        }
    }
}
