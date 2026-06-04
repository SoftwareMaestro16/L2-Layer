use crate::crypto::Hash32;
use crate::state::State;
use crate::types::{
    L2TransactionKind, Receipt, SignedL2Transaction, WithdrawalLeaf, L2_NATIVE_GAS_ASSET,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub block_time: u64,
    pub block_height: u64,
    pub gas_coin_asset: u32,
    pub max_internal_messages: u32,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            block_time: 0,
            block_height: 0,
            gas_coin_asset: L2_NATIVE_GAS_ASSET,
            max_internal_messages: 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub receipt: Receipt,
    pub withdrawals: Vec<WithdrawalLeaf>,
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
        let tx_hash = tx.tx_hash();

        match &tx.kind {
            L2TransactionKind::Deposit {
                asset_id,
                recipient,
                amount,
                ..
            } => {
                let account = state.account_mut(*recipient);
                if !account.credit(*asset_id, *amount) {
                    return rejected(tx_hash, "balance_overflow");
                }
                account.last_lt = config.block_height;
                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, 0, None),
                    withdrawals: vec![],
                }
            }
            L2TransactionKind::Transfer {
                to,
                asset_id,
                amount,
            } => {
                let Some(from) = tx.from else {
                    return rejected(tx_hash, "missing_sender");
                };
                let gas = gas_cost(tx);
                let recipient_can_credit = state
                    .account(*to)
                    .map_or(true, |account| account.can_credit(*asset_id, *amount));
                if !recipient_can_credit {
                    return rejected(tx_hash, "balance_overflow");
                }
                if !debit_total(state, from, *asset_id, *amount, config.gas_coin_asset, gas) {
                    return rejected(tx_hash, "insufficient_balance");
                }

                {
                    let sender = state.account_mut(from);
                    sender.nonce += 1;
                    sender.last_lt = config.block_height;
                }
                let recipient = state.account_mut(*to);
                if !recipient.credit(*asset_id, *amount) {
                    return rejected(tx_hash, "balance_overflow");
                }
                recipient.last_lt = config.block_height;

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, gas, None),
                    withdrawals: vec![],
                }
            }
            L2TransactionKind::Withdraw {
                asset_id,
                amount,
                l1_recipient,
            } => {
                let Some(from) = tx.from else {
                    return rejected(tx_hash, "missing_sender");
                };
                let gas = gas_cost(tx);
                if !debit_total(state, from, *asset_id, *amount, config.gas_coin_asset, gas) {
                    return rejected(tx_hash, "insufficient_balance");
                }

                let withdrawal =
                    WithdrawalLeaf::new(tx_hash, *asset_id, *amount, from, l1_recipient.clone());
                {
                    let sender = state.account_mut(from);
                    sender.nonce += 1;
                    sender.last_lt = config.block_height;
                }

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, gas, Some(withdrawal.withdrawal_id)),
                    withdrawals: vec![withdrawal],
                }
            }
            L2TransactionKind::CallContract { .. } => {
                rejected(tx_hash, "tvm_adapter_not_implemented")
            }
        }
    }
}

fn rejected(tx_hash: Hash32, reason: impl Into<String>) -> ExecutionOutcome {
    ExecutionOutcome {
        receipt: Receipt::rejected(tx_hash, reason),
        withdrawals: vec![],
    }
}

fn gas_cost(tx: &SignedL2Transaction) -> u128 {
    match tx.kind {
        L2TransactionKind::Deposit { .. } => 0,
        L2TransactionKind::Transfer { .. } => 10,
        L2TransactionKind::Withdraw { .. } => 20,
        L2TransactionKind::CallContract { .. } => 50,
    }
}

fn debit_total(
    state: &mut State,
    from: Hash32,
    asset_id: u32,
    amount: u128,
    gas_asset_id: u32,
    gas: u128,
) -> bool {
    let account = state.account_mut(from);
    if asset_id == gas_asset_id {
        let total = match amount.checked_add(gas) {
            Some(total) => total,
            None => return false,
        };
        account.debit(asset_id, total)
    } else if account.balance(gas_asset_id) >= gas && account.balance(asset_id) >= amount {
        account.debit(gas_asset_id, gas) && account.debit(asset_id, amount)
    } else {
        false
    }
}
