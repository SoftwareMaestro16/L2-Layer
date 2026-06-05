use crate::address::is_l2_zero_address;
use crate::batch::{BatchBuildInput, BatchBuilder};
use crate::crypto::{decode_public_key, verify_signature, Hash32};
use crate::executor::{DeterministicExecutor, ExecutionConfig, TvmAdapterMode};
use crate::gas::GasSchedule;
use crate::internal_queue::{
    InternalMessageQueue, InternalMessageQueueSnapshot, DEFAULT_INTERNAL_MESSAGE_GAS_LIMIT,
    DEFAULT_MAX_INTERNAL_MESSAGES_PER_BLOCK, DEFAULT_MAX_INTERNAL_QUEUE_LEN,
};
use crate::sequencer_validation::{
    validate_account_public_key, validate_public_sender_account, validate_reserved_zero_addresses,
    validate_tx_envelope,
};
use crate::state::State;
use crate::tvm::TvmExecutionAdapter;
use crate::types::{
    DepositEvent, L2Block, L2BlockHeader, L2TransactionKind, Receipt, SignedL2Transaction,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SequencerConfig {
    pub chain_id: String,
    pub max_txs_per_block: usize,
    pub block_gas_limit: u64,
    pub gas_coin_asset: u32,
    pub gas_schedule: GasSchedule,
    pub max_internal_messages: u32,
    pub max_internal_queue_len: usize,
    pub max_internal_messages_per_block: usize,
    pub internal_message_gas_limit: u64,
    pub tvm_adapter_mode: TvmAdapterMode,
    pub tvm_tonlib_library_path: Option<PathBuf>,
}

impl Default for SequencerConfig {
    fn default() -> Self {
        Self {
            chain_id: "ton-l2-devnet".to_owned(),
            max_txs_per_block: 1024,
            block_gas_limit: 1_000_000,
            gas_coin_asset: crate::types::L2_NATIVE_GAS_ASSET,
            gas_schedule: GasSchedule::default(),
            max_internal_messages: 1024,
            max_internal_queue_len: DEFAULT_MAX_INTERNAL_QUEUE_LEN,
            max_internal_messages_per_block: DEFAULT_MAX_INTERNAL_MESSAGES_PER_BLOCK,
            internal_message_gas_limit: DEFAULT_INTERNAL_MESSAGE_GAS_LIMIT,
            tvm_adapter_mode: TvmAdapterMode::Real,
            tvm_tonlib_library_path: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mempool {
    pending: VecDeque<QueuedTransaction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransactionOrigin {
    User,
    System,
}

#[derive(Clone, Debug)]
struct QueuedTransaction {
    tx: SignedL2Transaction,
    origin: TransactionOrigin,
}

impl Mempool {
    pub fn submit(&mut self, tx: SignedL2Transaction) -> Hash32 {
        let hash = tx.tx_hash();
        self.pending.push_back(QueuedTransaction {
            tx,
            origin: TransactionOrigin::User,
        });
        hash
    }

    pub fn insert_system_deposits(&mut self, chain_id: &str, deposits: Vec<DepositEvent>) {
        for deposit in deposits {
            self.pending.push_back(
                SignedL2Transaction::system_deposit(
                    chain_id,
                    deposit.deposit_id,
                    deposit.asset_id,
                    deposit.recipient,
                    deposit.amount,
                )
                .into_system_queue_entry(),
            );
        }
    }

    fn select_block(&mut self, max_txs: usize) -> Vec<QueuedTransaction> {
        let mut out = Vec::with_capacity(max_txs.min(self.pending.len()));
        for _ in 0..max_txs {
            let Some(tx) = self.pending.pop_front() else {
                break;
            };
            out.push(tx);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

trait IntoSystemQueueEntry {
    fn into_system_queue_entry(self) -> QueuedTransaction;
}

impl IntoSystemQueueEntry for SignedL2Transaction {
    fn into_system_queue_entry(self) -> QueuedTransaction {
        QueuedTransaction {
            tx: self,
            origin: TransactionOrigin::System,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sequencer {
    pub config: SequencerConfig,
    pub state: State,
    pub mempool: Mempool,
    internal_queue: InternalMessageQueue,
    executor: DeterministicExecutor,
    committed_deposits: BTreeSet<Hash32>,
    last_header: Option<L2BlockHeader>,
}

impl Sequencer {
    pub fn new(config: SequencerConfig) -> Self {
        let internal_queue = InternalMessageQueue::new(config.max_internal_queue_len);
        Self {
            config,
            state: State::default(),
            mempool: Mempool::default(),
            internal_queue,
            executor: DeterministicExecutor,
            committed_deposits: BTreeSet::new(),
            last_header: None,
        }
    }

    pub fn submit_tx(&mut self, tx: SignedL2Transaction) -> Hash32 {
        self.mempool.submit(tx)
    }

    pub fn ingest_deposits(&mut self, deposits: Vec<DepositEvent>) {
        let new_deposits = deposits
            .into_iter()
            .filter(|deposit| self.committed_deposits.insert(deposit.deposit_id))
            .collect::<Vec<_>>();
        self.mempool
            .insert_system_deposits(&self.config.chain_id, new_deposits);
    }

    pub fn produce_block(&mut self, timestamp: u64) -> Option<L2Block> {
        match self.config.tvm_adapter_mode {
            TvmAdapterMode::Real => {
                let mut backend = crate::tvm::TonlibTvmBackend::default();
                if let Some(path) = self.config.tvm_tonlib_library_path.as_ref() {
                    backend = backend.with_library_path(path.clone());
                }
                let tvm_adapter = crate::tvm::RealTvmAdapter::new(backend);
                self.produce_block_with_tvm_adapter(timestamp, &tvm_adapter)
            }
            TvmAdapterMode::Prototype => {
                let tvm_adapter = crate::tvm::PrototypeTvmAdapter;
                self.produce_block_with_tvm_adapter(timestamp, &tvm_adapter)
            }
        }
    }

    #[cfg(test)]
    pub fn produce_block_with_test_tvm_adapter<A: TvmExecutionAdapter + ?Sized>(
        &mut self,
        timestamp: u64,
        tvm_adapter: &A,
    ) -> Option<L2Block> {
        self.produce_block_with_tvm_adapter(timestamp, tvm_adapter)
    }

    pub fn pending_internal_message_count(&self) -> usize {
        self.internal_queue.len()
    }

    pub fn internal_queue_snapshot(&self) -> InternalMessageQueueSnapshot {
        self.internal_queue.snapshot()
    }

    pub fn restore_internal_queue(
        &mut self,
        snapshot: InternalMessageQueueSnapshot,
    ) -> Result<(), crate::InternalMessageQueueError> {
        self.internal_queue =
            InternalMessageQueue::from_snapshot(self.config.max_internal_queue_len, snapshot)?;
        Ok(())
    }

    fn produce_block_with_tvm_adapter<A: TvmExecutionAdapter + ?Sized>(
        &mut self,
        timestamp: u64,
        tvm_adapter: &A,
    ) -> Option<L2Block> {
        if self.mempool.is_empty() && self.internal_queue.is_empty() {
            return None;
        }

        let block_height = self.next_height();
        let prev_state_root = self.state.root_hash();
        let ready_internal_at_block_start = self.internal_queue.len();
        let queued_txs = self.mempool.select_block(self.config.max_txs_per_block);
        let mut transactions = Vec::with_capacity(self.config.max_txs_per_block);
        let mut receipts = Vec::with_capacity(self.config.max_txs_per_block);
        let mut withdrawals = Vec::new();
        let mut block_gas_used = 0u64;
        let mut seen_tx_hashes = BTreeSet::new();

        for queued in queued_txs {
            let tx_hash = queued.tx.tx_hash();
            if !seen_tx_hashes.insert(tx_hash) {
                receipts.push(Receipt::rejected(tx_hash, "duplicate_tx"));
                transactions.push(queued.tx);
                continue;
            }
            if let Err(reason) = self.verify_tx(&queued.tx, queued.origin, block_height) {
                receipts.push(Receipt::rejected(queued.tx.tx_hash(), reason));
                transactions.push(queued.tx);
                continue;
            }

            let required_gas = self.config.gas_schedule.required_gas(&queued.tx.kind);
            let Some(next_block_gas_used) = block_gas_used.checked_add(required_gas) else {
                receipts.push(Receipt::rejected(
                    queued.tx.tx_hash(),
                    "block_gas_limit_exceeded",
                ));
                transactions.push(queued.tx);
                continue;
            };
            if next_block_gas_used > self.config.block_gas_limit {
                receipts.push(Receipt::rejected(
                    queued.tx.tx_hash(),
                    "block_gas_limit_exceeded",
                ));
                transactions.push(queued.tx);
                continue;
            }

            let state_before = self.state.clone();
            let outcome = self.executor.apply_with_tvm_adapter(
                &mut self.state,
                &queued.tx,
                &ExecutionConfig {
                    block_time: timestamp,
                    block_height,
                    gas_coin_asset: self.config.gas_coin_asset,
                    gas_schedule: self.config.gas_schedule,
                    max_internal_messages: self.config.max_internal_messages,
                    tvm_adapter_mode: self.config.tvm_adapter_mode.clone(),
                    tvm_tonlib_library_path: self.config.tvm_tonlib_library_path.clone(),
                    ..ExecutionConfig::default()
                },
                tvm_adapter,
            );
            block_gas_used = next_block_gas_used;
            let receipt = match self
                .internal_queue
                .push_many(block_height, outcome.internal_messages)
            {
                Ok(()) => {
                    withdrawals.extend(outcome.withdrawals);
                    outcome.receipt
                }
                Err(error) => {
                    self.state = state_before;
                    Receipt::rejected(tx_hash, error.rejection_reason())
                }
            };
            receipts.push(receipt);
            transactions.push(queued.tx);
        }

        let remaining_tx_capacity = self
            .config
            .max_txs_per_block
            .saturating_sub(transactions.len());
        let internal_limit = remaining_tx_capacity
            .min(self.config.max_internal_messages_per_block)
            .min(ready_internal_at_block_start);
        for _ in 0..internal_limit {
            let required_gas = self.config.gas_schedule.call_contract_gas;
            let Some(next_block_gas_used) = block_gas_used.checked_add(required_gas) else {
                break;
            };
            if next_block_gas_used > self.config.block_gas_limit {
                break;
            }
            let Some(message) = self.internal_queue.pop_front() else {
                break;
            };
            let tx = SignedL2Transaction::system_internal_message(
                &self.config.chain_id,
                message.message_id,
                message.message.from,
                message.message.to,
                message.message.value,
                message.message.body_boc,
                message.message.bounce,
                message.message.bounced,
                self.config.internal_message_gas_limit,
            );
            let tx_hash = tx.tx_hash();
            if !seen_tx_hashes.insert(tx_hash) {
                receipts.push(Receipt::rejected(tx_hash, "duplicate_tx"));
                transactions.push(tx);
                block_gas_used = next_block_gas_used;
                continue;
            }

            let state_before = self.state.clone();
            let outcome = self.executor.apply_with_tvm_adapter(
                &mut self.state,
                &tx,
                &ExecutionConfig {
                    block_time: timestamp,
                    block_height,
                    gas_coin_asset: self.config.gas_coin_asset,
                    gas_schedule: self.config.gas_schedule,
                    max_internal_messages: self.config.max_internal_messages,
                    tvm_adapter_mode: self.config.tvm_adapter_mode.clone(),
                    tvm_tonlib_library_path: self.config.tvm_tonlib_library_path.clone(),
                    ..ExecutionConfig::default()
                },
                tvm_adapter,
            );
            block_gas_used = next_block_gas_used;
            let receipt = match self
                .internal_queue
                .push_many(block_height, outcome.internal_messages)
            {
                Ok(()) => outcome.receipt,
                Err(error) => {
                    self.state = state_before;
                    Receipt::rejected(tx_hash, error.rejection_reason())
                }
            };
            receipts.push(receipt);
            transactions.push(tx);
        }

        if transactions.is_empty() {
            return None;
        }

        let state_root = self.state.root_hash();
        let block = BatchBuilder::build(BatchBuildInput {
            previous_header: self.last_header.clone(),
            prev_state_root,
            state_root,
            ordered_transactions: transactions,
            receipts,
            withdrawals,
            timestamp,
        })
        .expect("sequencer emits exactly one receipt per selected transaction");

        self.last_header = Some(block.header.clone());
        Some(block)
    }

    fn next_height(&self) -> u64 {
        self.last_header
            .as_ref()
            .map_or(0, |header| header.height + 1)
    }

    fn verify_tx(
        &self,
        tx: &SignedL2Transaction,
        origin: TransactionOrigin,
        block_height: u64,
    ) -> Result<(), &'static str> {
        if tx.chain_id != self.config.chain_id {
            return Err("wrong_chain_id");
        }
        validate_tx_envelope(tx, block_height)?;

        if origin == TransactionOrigin::System {
            if !tx.is_system() {
                return Err("system_tx_required");
            }
            if tx.from.is_some() || tx.public_key.is_some() || tx.signature.is_some() {
                return Err("invalid_system_tx_auth");
            }
            return validate_reserved_zero_addresses(tx, true);
        }

        if matches!(tx.kind, L2TransactionKind::Deposit { .. }) {
            return Err("deposit_must_be_system");
        }
        if matches!(tx.kind, L2TransactionKind::InternalMessage { .. }) {
            return Err("internal_message_must_be_system");
        }
        if tx.fee_asset_id != self.config.gas_coin_asset {
            return Err("unsupported_fee_asset");
        }

        let from = tx.from.ok_or("missing_sender")?;
        if is_l2_zero_address(from) {
            return Err("reserved_zero_address");
        }
        let public_key_hex = tx.public_key.as_deref().ok_or("missing_public_key")?;
        let signature_hex = tx.signature.as_deref().ok_or("missing_signature")?;
        let account = self.state.account(from).ok_or("unknown_sender")?;
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
}

#[cfg(test)]
#[path = "sequencer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sequencer_internal_tests.rs"]
mod internal_tests;
