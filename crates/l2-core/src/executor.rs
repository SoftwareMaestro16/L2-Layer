use crate::address::is_l2_zero_address;
use crate::crypto::{decode_public_key, Hash32};
use crate::gas::{GasFee, GasSchedule};
use crate::state::{Account, AccountType, State};
use crate::tvm::{
    decode_contract_cell_boc_base64, ContractCellField, TvmExecutionAdapter, TvmInternalMessage,
    DEFAULT_MAX_CONTRACT_CODE_BOC_BYTES, DEFAULT_MAX_CONTRACT_DATA_BOC_BYTES,
    DEFAULT_MAX_TVM_BOC_BYTES,
};
use crate::types::{
    L2Event, L2TransactionKind, Receipt, SignedL2Transaction, WithdrawalLeaf, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use crate::withdrawal::validate_release_parts;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[path = "executor/tvm_call.rs"]
mod tvm_call;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TvmAdapterMode {
    #[default]
    Real,
    Prototype,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub block_time: u64,
    pub block_height: u64,
    pub gas_coin_asset: u32,
    pub gas_schedule: GasSchedule,
    pub max_internal_messages: u32,
    pub max_tvm_boc_bytes: usize,
    pub max_contract_code_boc_bytes: usize,
    pub max_contract_data_boc_bytes: usize,
    pub tvm_adapter_mode: TvmAdapterMode,
    pub tvm_tonlib_library_path: Option<PathBuf>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            block_time: 0,
            block_height: 0,
            gas_coin_asset: L2_NATIVE_GAS_ASSET,
            gas_schedule: GasSchedule::default(),
            max_internal_messages: 1024,
            max_tvm_boc_bytes: DEFAULT_MAX_TVM_BOC_BYTES,
            max_contract_code_boc_bytes: DEFAULT_MAX_CONTRACT_CODE_BOC_BYTES,
            max_contract_data_boc_bytes: DEFAULT_MAX_CONTRACT_DATA_BOC_BYTES,
            tvm_adapter_mode: TvmAdapterMode::Real,
            tvm_tonlib_library_path: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub receipt: Receipt,
    pub withdrawals: Vec<WithdrawalLeaf>,
    pub internal_messages: Vec<TvmInternalMessage>,
}

#[derive(Clone, Debug, Default)]
pub struct DeterministicExecutor;

impl DeterministicExecutor {
    pub fn apply(
        &self,
        state: &mut State,
        tx: &SignedL2Transaction,
        config: &ExecutionConfig,
    ) -> ExecutionOutcome {
        match config.tvm_adapter_mode {
            TvmAdapterMode::Real => {
                let mut backend = crate::tvm::TonlibTvmBackend::default();
                if let Some(path) = config.tvm_tonlib_library_path.as_ref() {
                    backend = backend.with_library_path(path.clone());
                }
                let tvm_adapter = crate::tvm::RealTvmAdapter::new(backend);
                self.apply_with_tvm_adapter(state, tx, config, &tvm_adapter)
            }
            TvmAdapterMode::Prototype => {
                let tvm_adapter = crate::tvm::PrototypeTvmAdapter;
                self.apply_with_tvm_adapter(state, tx, config, &tvm_adapter)
            }
        }
    }

    pub fn apply_with_tvm_adapter<A: TvmExecutionAdapter + ?Sized>(
        &self,
        state: &mut State,
        tx: &SignedL2Transaction,
        config: &ExecutionConfig,
        tvm_adapter: &A,
    ) -> ExecutionOutcome {
        let tx_hash = tx.tx_hash();
        if let Err(reason) = validate_execution_envelope(tx, config) {
            return rejected(tx_hash, reason);
        }

        match &tx.kind {
            L2TransactionKind::Deposit {
                asset_id,
                recipient,
                amount,
                ..
            } => {
                if is_l2_zero_address(*recipient) {
                    return rejected(tx_hash, "reserved_zero_address");
                }
                let account = state.account_mut(*recipient);
                if !account.credit(*asset_id, *amount) {
                    return rejected(tx_hash, "balance_overflow");
                }
                account.last_lt = config.block_height;
                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, 0, None),
                    withdrawals: vec![],
                    internal_messages: vec![],
                }
            }
            L2TransactionKind::Transfer {
                to,
                asset_id,
                amount,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                if is_l2_zero_address(*to) {
                    return rejected_attempt(state, tx, from, config, "reserved_zero_address");
                }
                let recipient_can_credit = state
                    .account(*to)
                    .map_or(true, |account| account.can_credit(*asset_id, *amount));
                if !recipient_can_credit {
                    return rejected_attempt(state, tx, from, config, "balance_overflow");
                }
                if let Err(reason) =
                    validate_total_debit(state, from, *asset_id, *amount, tx.fee_asset_id, fee)
                {
                    return rejected_attempt(state, tx, from, config, reason);
                }
                if !debit_total(state, from, *asset_id, *amount, tx.fee_asset_id, fee) {
                    return rejected_attempt(state, tx, from, config, "insufficient_balance");
                }

                {
                    let sender = state.account_mut(from);
                    mark_sender_attempt(sender, tx, config.block_height);
                }
                let recipient = state.account_mut(*to);
                if !recipient.credit(*asset_id, *amount) {
                    return rejected_attempt(state, tx, from, config, "balance_overflow");
                }
                recipient.last_lt = config.block_height;

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, None),
                    withdrawals: vec![],
                    internal_messages: vec![],
                }
            }
            L2TransactionKind::RotatePublicKey { new_public_key } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                let new_public_key = match decode_public_key(new_public_key) {
                    Ok(public_key) => public_key,
                    Err(_) => {
                        return rejected_attempt(state, tx, from, config, "invalid_public_key");
                    }
                };
                if state
                    .account(from)
                    .is_none_or(|account| account.balance(tx.fee_asset_id) < fee.amount)
                {
                    return rejected_attempt(state, tx, from, config, "insufficient_gas_coin");
                }
                {
                    let sender = state.account_mut(from);
                    if !sender.debit(tx.fee_asset_id, fee.amount) {
                        return rejected_attempt(state, tx, from, config, "insufficient_gas_coin");
                    }
                    mark_sender_attempt(sender, tx, config.block_height);
                    sender.active_public_key = Some(Hash32::new(new_public_key));
                }

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, None),
                    withdrawals: vec![],
                    internal_messages: vec![],
                }
            }
            L2TransactionKind::Withdraw {
                asset_id,
                amount,
                l1_recipient,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                if let Err(error) = validate_release_parts(*amount, l1_recipient) {
                    return rejected_attempt(state, tx, from, config, error.rejection_reason());
                }
                if let Err(reason) =
                    validate_total_debit(state, from, *asset_id, *amount, tx.fee_asset_id, fee)
                {
                    return rejected_attempt(state, tx, from, config, reason);
                }
                if !debit_total(state, from, *asset_id, *amount, tx.fee_asset_id, fee) {
                    return rejected_attempt(state, tx, from, config, "insufficient_balance");
                }

                let withdrawal =
                    WithdrawalLeaf::new(tx_hash, *asset_id, *amount, from, l1_recipient.clone());
                {
                    let sender = state.account_mut(from);
                    mark_sender_attempt(sender, tx, config.block_height);
                }

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, Some(withdrawal.withdrawal_id))
                        .with_events(vec![L2Event::WithdrawalCreated {
                            withdrawal_id: withdrawal.withdrawal_id,
                            asset_id: withdrawal.asset_id,
                            amount: withdrawal.amount,
                            l2_sender: withdrawal.l2_sender,
                            l1_recipient: withdrawal.l1_recipient.clone(),
                        }]),
                    withdrawals: vec![withdrawal],
                    internal_messages: vec![],
                }
            }
            L2TransactionKind::DeployContract {
                contract,
                code_boc_base64,
                data_boc_base64,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                if is_l2_zero_address(*contract) {
                    return rejected_attempt(state, tx, from, config, "reserved_zero_address");
                }
                let code_cell = match decode_contract_cell_boc_base64(
                    code_boc_base64,
                    config.max_contract_code_boc_bytes,
                ) {
                    Ok(cell) => cell,
                    Err(error) => {
                        return rejected_attempt(
                            state,
                            tx,
                            from,
                            config,
                            error.deploy_reason(ContractCellField::Code),
                        );
                    }
                };
                let data_cell = match decode_contract_cell_boc_base64(
                    data_boc_base64,
                    config.max_contract_data_boc_bytes,
                ) {
                    Ok(cell) => cell,
                    Err(error) => {
                        return rejected_attempt(
                            state,
                            tx,
                            from,
                            config,
                            error.deploy_reason(ContractCellField::Data),
                        );
                    }
                };
                if state
                    .account(*contract)
                    .is_some_and(|account| !account.can_initialize_contract())
                {
                    return rejected_attempt(state, tx, from, config, "contract_already_exists");
                }
                if state
                    .account(from)
                    .is_none_or(|account| account.balance(tx.fee_asset_id) < fee.amount)
                {
                    return rejected_attempt(state, tx, from, config, "insufficient_gas_coin");
                }
                {
                    let sender = state.account_mut(from);
                    if !sender.debit(tx.fee_asset_id, fee.amount) {
                        return rejected_attempt(state, tx, from, config, "insufficient_gas_coin");
                    }
                    mark_sender_attempt(sender, tx, config.block_height);
                }
                let deployed = state.account_mut(*contract);
                deployed.mark_contract_account();
                deployed.code_hash = code_cell.cell_hash;
                deployed.data_hash = data_cell.cell_hash;
                deployed.storage_root = data_cell.cell_hash;
                deployed.code_boc_base64 = Some(code_cell.boc_base64);
                deployed.data_boc_base64 = Some(data_cell.boc_base64);
                deployed.last_lt = config.block_height;

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, None).with_events(vec![
                        L2Event::ContractDeployed {
                            contract: *contract,
                            deployer: from,
                            code_hash: code_cell.cell_hash,
                            data_hash: data_cell.cell_hash,
                        },
                    ]),
                    withdrawals: vec![],
                    internal_messages: vec![],
                }
            }
            L2TransactionKind::InternalMessage {
                message_id,
                from,
                to,
                value,
                body_boc_base64,
                bounce,
                bounced,
            } => tvm_call::execute_internal_message(
                state,
                tx_hash,
                *message_id,
                *from,
                *to,
                *value,
                body_boc_base64,
                *bounce,
                *bounced,
                tx.gas_limit,
                config,
                tvm_adapter,
            ),
            L2TransactionKind::CallContract {
                contract,
                body_boc_base64,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                tvm_call::execute_contract_call(
                    state,
                    tx,
                    from,
                    *contract,
                    body_boc_base64,
                    config,
                    tvm_adapter,
                )
            }
        }
    }
}

fn rejected(tx_hash: Hash32, reason: impl Into<String>) -> ExecutionOutcome {
    ExecutionOutcome {
        receipt: Receipt::rejected(tx_hash, reason),
        withdrawals: vec![],
        internal_messages: vec![],
    }
}

fn validate_execution_envelope(
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
) -> Result<(), &'static str> {
    if tx.tx_version != L2_TX_VERSION_V2 {
        return Err("unsupported_tx_version");
    }
    if tx.domain_separator != L2_TX_DOMAIN_SEPARATOR {
        return Err("invalid_domain_separator");
    }
    if tx.transaction_kind_version != L2_TRANSACTION_KIND_VERSION_V1 {
        return Err("unsupported_transaction_kind_version");
    }
    if tx.valid_until_block < config.block_height {
        return Err("tx_expired");
    }
    if !tx.is_system() && tx.fee_asset_id != config.gas_coin_asset {
        return Err("unsupported_fee_asset");
    }
    Ok(())
}

fn rejected_attempt(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
    reason: impl Into<String>,
) -> ExecutionOutcome {
    let tx_hash = tx.tx_hash();
    let gas_charged = charge_rejection_fee(state, tx, from, config);
    ExecutionOutcome {
        receipt: Receipt::rejected_with_gas(tx_hash, reason, gas_charged),
        withdrawals: vec![],
        internal_messages: vec![],
    }
}

fn execution_fee(
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
) -> Result<GasFee, crate::gas::GasError> {
    config
        .gas_schedule
        .execution_fee(&tx.kind, tx.gas_limit, tx.max_gas_price)
}

fn authenticated_sender(state: &State, tx: &SignedL2Transaction) -> Result<Hash32, &'static str> {
    let from = tx.from.ok_or("missing_sender")?;
    if is_l2_zero_address(from) {
        return Err("reserved_zero_address");
    }
    let account = state.account(from).ok_or("unknown_sender")?;
    if let Some(reason) = public_sender_rejection(account) {
        return Err(reason);
    }
    Ok(from)
}

fn public_sender_rejection(account: &Account) -> Option<&'static str> {
    if account.flags.disabled {
        return Some("account_disabled");
    }
    if account.is_recovery_locked() {
        return Some("account_recovery_locked");
    }
    if account.flags.system_only || matches!(account.account_type, AccountType::System) {
        return Some("sender_system_only");
    }
    if account.flags.contract_only || matches!(account.account_type, AccountType::Contract) {
        return Some("sender_contract_only");
    }
    if !account.can_send_public_transaction() {
        return Some("sender_not_public");
    }
    None
}

fn can_increment_nonce(state: &State, from: Hash32) -> bool {
    state
        .account(from)
        .is_some_and(|account| account.nonce < u64::MAX)
}

fn mark_sender_attempt(
    account: &mut crate::state::Account,
    tx: &SignedL2Transaction,
    block_height: u64,
) {
    if account.active_public_key.is_none() {
        if let Some(public_key) = tx
            .public_key
            .as_deref()
            .and_then(|public_key| decode_public_key(public_key).ok())
        {
            account.active_public_key = Some(Hash32::new(public_key));
        }
    }
    account.nonce += 1;
    account.last_lt = block_height;
}

fn charge_rejection_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
) -> u128 {
    if !can_increment_nonce(state, from) {
        return 0;
    }
    let fee = config
        .gas_schedule
        .rejection_fee(tx.gas_limit, tx.max_gas_price)
        .map_or(0, |fee| fee.amount);
    let account = state.account_mut(from);
    let gas_charged = if fee > 0 && account.balance(tx.fee_asset_id) >= fee {
        if account.debit(tx.fee_asset_id, fee) {
            fee
        } else {
            0
        }
    } else {
        0
    };
    mark_sender_attempt(account, tx, config.block_height);
    gas_charged
}

fn validate_total_debit(
    state: &State,
    from: Hash32,
    asset_id: u32,
    amount: u128,
    gas_asset_id: u32,
    fee: GasFee,
) -> Result<(), &'static str> {
    let Some(account) = state.account(from) else {
        return Err("unknown_sender");
    };
    if asset_id == gas_asset_id {
        let total = amount.checked_add(fee.amount).ok_or("fee_overflow")?;
        if account.balance(asset_id) < total {
            return Err("insufficient_balance");
        }
    } else {
        if account.balance(gas_asset_id) < fee.amount {
            return Err("insufficient_gas_coin");
        }
        if account.balance(asset_id) < amount {
            return Err("insufficient_balance");
        }
    }
    Ok(())
}

fn debit_total(
    state: &mut State,
    from: Hash32,
    asset_id: u32,
    amount: u128,
    gas_asset_id: u32,
    fee: GasFee,
) -> bool {
    let account = state.account_mut(from);
    if asset_id == gas_asset_id {
        let total = match amount.checked_add(fee.amount) {
            Some(total) => total,
            None => return false,
        };
        account.debit(asset_id, total)
    } else if account.balance(gas_asset_id) >= fee.amount && account.balance(asset_id) >= amount {
        account.debit(gas_asset_id, fee.amount) && account.debit(asset_id, amount)
    } else {
        false
    }
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "executor_tvm_tests.rs"]
mod tvm_tests;
