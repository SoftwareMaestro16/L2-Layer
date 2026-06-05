use super::validation::{
    can_increment_nonce, charge_rejection_fee, execution_fee, mark_sender_attempt, rejected,
    rejected_attempt,
};
use super::{ExecutionConfig, ExecutionOutcome};
use crate::address::is_l2_zero_address;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::{AccountType, State};
use crate::tvm::{
    decode_call_body_boc_base64, validate_tvm_output, TvmBoundaryError, TvmExecutionAdapter,
    TvmExecutionContext, TvmExecutionInput, TvmExecutionStatus, TvmInternalMessage, TvmStateDelta,
};
use crate::types::{L2Event, Receipt, SignedL2Transaction};
use tonlib_core::cell::{BagOfCells, CellBuilder};

pub(super) fn execute_contract_call<A: TvmExecutionAdapter + ?Sized>(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    contract: Hash32,
    body_boc_base64: &str,
    config: &ExecutionConfig,
    tvm_adapter: &A,
) -> ExecutionOutcome {
    let tx_hash = tx.tx_hash();
    if let Err(error) = execution_fee(tx, config) {
        return rejected_attempt(state, tx, from, config, error.rejection_reason());
    }
    if !can_increment_nonce(state, from) {
        return rejected(tx_hash, "nonce_overflow");
    }
    if is_l2_zero_address(contract) {
        return rejected_attempt(state, tx, from, config, "reserved_zero_address");
    }
    if let Err(reason) = validate_max_call_fee(state, tx, from, config) {
        return rejected_attempt(state, tx, from, config, reason);
    }
    let input_boc = match decode_call_body_boc_base64(body_boc_base64, config.max_tvm_boc_bytes) {
        Ok(input_boc) => input_boc,
        Err(error) => return rejected_attempt(state, tx, from, config, error.rejection_reason()),
    };
    let body_hash = sha256_bytes(&input_boc);
    let Some(contract_account) = state.account(contract) else {
        return rejected_attempt(
            state,
            tx,
            from,
            config,
            TvmBoundaryError::UnknownContract.rejection_reason(),
        );
    };
    if contract_account.code_hash == Hash32::ZERO {
        return rejected_attempt(
            state,
            tx,
            from,
            config,
            TvmBoundaryError::ContractCodeMissing.rejection_reason(),
        );
    }
    if contract_account.code_boc_base64.is_none() || contract_account.data_boc_base64.is_none() {
        return rejected_attempt(
            state,
            tx,
            from,
            config,
            TvmBoundaryError::ContractCodeMissing.rejection_reason(),
        );
    }

    let input = TvmExecutionInput {
        caller: from,
        contract,
        input_boc,
        gas_limit: tx.gas_limit,
        context: TvmExecutionContext {
            block_time: config.block_time,
            block_height: config.block_height,
            gas_coin_asset: config.gas_coin_asset,
            max_internal_messages: config.max_internal_messages,
        },
        contract_state: contract_account.into(),
    };
    let output = match tvm_adapter.execute(&input) {
        Ok(output) => output,
        Err(error) => return rejected_attempt(state, tx, from, config, error.rejection_reason()),
    };

    if let Err(error) = validate_tvm_output(
        &output,
        contract,
        tx.gas_limit,
        config.max_internal_messages,
        config.max_tvm_boc_bytes,
    ) {
        let gas_charged = charge_call_or_rejection_fee(state, tx, from, config, output.gas_used);
        return ExecutionOutcome {
            receipt: Receipt::rejected_with_gas(tx_hash, error.rejection_reason(), gas_charged),
            withdrawals: vec![],
            internal_messages: vec![],
        };
    }

    let gas_charged = match charge_call_fee(state, tx, from, config, output.gas_used) {
        Ok(gas_charged) => gas_charged,
        Err(reason) => return rejected_attempt(state, tx, from, config, reason),
    };

    match output.status {
        TvmExecutionStatus::Applied => {
            if let Some(delta) = output.state_delta {
                apply_tvm_state_delta(state, contract, delta, config.block_height);
            }
            ExecutionOutcome {
                receipt: Receipt::applied(tx_hash, gas_charged, None).with_events(vec![
                    L2Event::ContractCalled {
                        contract,
                        caller: from,
                        body_hash,
                    },
                ]),
                withdrawals: vec![],
                internal_messages: output.emitted_internal_messages,
            }
        }
        TvmExecutionStatus::Rejected { reason } => ExecutionOutcome {
            receipt: Receipt::rejected_with_gas(tx_hash, reason, gas_charged),
            withdrawals: vec![],
            internal_messages: vec![],
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_internal_message<A: TvmExecutionAdapter + ?Sized>(
    state: &mut State,
    tx_hash: Hash32,
    message_id: Hash32,
    from: Hash32,
    to: Hash32,
    value: u128,
    body_boc_base64: &str,
    bounce: bool,
    bounced: bool,
    gas_limit: u64,
    config: &ExecutionConfig,
    tvm_adapter: &A,
) -> ExecutionOutcome {
    if message_id == Hash32::ZERO || is_l2_zero_address(from) || is_l2_zero_address(to) {
        return internal_rejected(tx_hash, None, "reserved_zero_address");
    }
    let body_boc = match decode_call_body_boc_base64(body_boc_base64, config.max_tvm_boc_bytes) {
        Ok(body_boc) => body_boc,
        Err(error) => {
            return internal_rejected(
                tx_hash,
                bounce_message(from, to, Vec::new(), bounce, bounced),
                error.rejection_reason(),
            );
        }
    };
    let body_hash = sha256_bytes(&body_boc);
    let original = TvmInternalMessage {
        from,
        to,
        value,
        body_boc: body_boc.clone(),
        bounce,
        bounced,
    };
    if value != 0 {
        return internal_rejected(
            tx_hash,
            bounce_from_original(&original),
            "internal_value_not_supported",
        );
    }
    if !bounced {
        let Some(sender_account) = state.account(from) else {
            return internal_rejected(tx_hash, None, "internal_sender_unknown");
        };
        if !matches!(sender_account.account_type, AccountType::Contract) {
            return internal_rejected(tx_hash, None, "internal_sender_not_contract");
        }
    }
    let Some(contract_account) = state.account(to) else {
        return internal_rejected(
            tx_hash,
            bounce_from_original(&original),
            TvmBoundaryError::UnknownContract.rejection_reason(),
        );
    };
    if contract_account.code_hash == Hash32::ZERO {
        return internal_rejected(
            tx_hash,
            bounce_from_original(&original),
            TvmBoundaryError::ContractCodeMissing.rejection_reason(),
        );
    }
    if contract_account.code_boc_base64.is_none() || contract_account.data_boc_base64.is_none() {
        return internal_rejected(
            tx_hash,
            bounce_from_original(&original),
            TvmBoundaryError::ContractCodeMissing.rejection_reason(),
        );
    }

    let input = TvmExecutionInput {
        caller: from,
        contract: to,
        input_boc: body_boc,
        gas_limit,
        context: TvmExecutionContext {
            block_time: config.block_time,
            block_height: config.block_height,
            gas_coin_asset: config.gas_coin_asset,
            max_internal_messages: config.max_internal_messages,
        },
        contract_state: contract_account.into(),
    };
    let output = match tvm_adapter.execute(&input) {
        Ok(output) => output,
        Err(error) => {
            return internal_rejected(
                tx_hash,
                bounce_from_original(&original),
                error.rejection_reason(),
            );
        }
    };

    if let Err(error) = validate_tvm_output(
        &output,
        to,
        gas_limit,
        config.max_internal_messages,
        config.max_tvm_boc_bytes,
    ) {
        return internal_rejected(
            tx_hash,
            bounce_from_original(&original),
            error.rejection_reason(),
        );
    }

    match output.status {
        TvmExecutionStatus::Applied => {
            if let Some(delta) = output.state_delta {
                apply_tvm_state_delta(state, to, delta, config.block_height);
            }
            ExecutionOutcome {
                receipt: Receipt::applied(tx_hash, 0, None).with_events(vec![
                    L2Event::ContractCalled {
                        contract: to,
                        caller: from,
                        body_hash,
                    },
                ]),
                withdrawals: vec![],
                internal_messages: output.emitted_internal_messages,
            }
        }
        TvmExecutionStatus::Rejected { reason } => {
            internal_rejected(tx_hash, bounce_from_original(&original), reason)
        }
    }
}

fn validate_max_call_fee(
    state: &State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
) -> Result<(), &'static str> {
    let max_fee = config
        .gas_schedule
        .fee_for_units(tx.gas_limit, tx.max_gas_price)
        .map_err(|error| error.rejection_reason())?;
    let Some(account) = state.account(from) else {
        return Err("unknown_sender");
    };
    if account.balance(tx.fee_asset_id) < max_fee.amount {
        return Err("insufficient_gas_coin");
    }
    Ok(())
}

fn charge_call_or_rejection_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
    gas_used: u64,
) -> u128 {
    if gas_used == 0 || gas_used > tx.gas_limit {
        return charge_rejection_fee(state, tx, from, config);
    }
    charge_call_fee(state, tx, from, config, gas_used)
        .unwrap_or_else(|_| charge_rejection_fee(state, tx, from, config))
}

fn charge_call_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
    gas_used: u64,
) -> Result<u128, &'static str> {
    if !can_increment_nonce(state, from) {
        return Err("nonce_overflow");
    }
    let fee = config
        .gas_schedule
        .fee_for_units(gas_used, tx.max_gas_price)
        .map_err(|error| error.rejection_reason())?;
    let account = state.account_mut(from);
    if account.balance(tx.fee_asset_id) < fee.amount {
        return Err("insufficient_gas_coin");
    }
    if !account.debit(tx.fee_asset_id, fee.amount) {
        return Err("insufficient_gas_coin");
    }
    mark_sender_attempt(account, tx, config.block_height);
    Ok(fee.amount)
}

fn apply_tvm_state_delta(
    state: &mut State,
    contract: Hash32,
    delta: TvmStateDelta,
    block_height: u64,
) {
    let account = state.account_mut(contract);
    if let Some(code_hash) = delta.code_hash {
        account.code_hash = code_hash;
    }
    if let Some(code_boc_base64) = delta.code_boc_base64 {
        account.code_boc_base64 = Some(code_boc_base64);
    }
    if let Some(data_hash) = delta.data_hash {
        account.data_hash = data_hash;
    }
    if let Some(data_boc_base64) = delta.data_boc_base64 {
        account.data_boc_base64 = Some(data_boc_base64);
    }
    if let Some(storage_root) = delta.storage_root {
        account.storage_root = storage_root;
    }
    account.last_lt = block_height;
}

fn internal_rejected(
    tx_hash: Hash32,
    bounce: Option<TvmInternalMessage>,
    reason: impl Into<String>,
) -> ExecutionOutcome {
    ExecutionOutcome {
        receipt: Receipt::rejected(tx_hash, reason),
        withdrawals: vec![],
        internal_messages: bounce.into_iter().collect(),
    }
}

fn bounce_from_original(original: &TvmInternalMessage) -> Option<TvmInternalMessage> {
    bounce_message(
        original.from,
        original.to,
        original.body_boc.clone(),
        original.bounce,
        original.bounced,
    )
}

fn bounce_message(
    from: Hash32,
    to: Hash32,
    _body_boc: Vec<u8>,
    bounce: bool,
    bounced: bool,
) -> Option<TvmInternalMessage> {
    if !bounce || bounced {
        return None;
    }
    Some(TvmInternalMessage {
        from: to,
        to: from,
        value: 0,
        body_boc: default_bounce_body_boc(),
        bounce: false,
        bounced: true,
    })
}

fn default_bounce_body_boc() -> Vec<u8> {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, 0xffff_ffff)
        .expect("bounce opcode fits uint32");
    let cell = builder.build().expect("bounce body cell");
    BagOfCells::from_root(cell)
        .serialize(false)
        .expect("bounce body BoC must serialize")
}
