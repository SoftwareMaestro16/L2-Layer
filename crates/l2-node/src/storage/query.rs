use l2_core::{Hash32, L2TransactionKind, SignedL2Transaction};

pub(super) fn is_before_transaction_cursor(
    block_height: u64,
    tx_index: usize,
    before_height: Option<u64>,
    before_index: Option<usize>,
) -> bool {
    let Some(before_height) = before_height else {
        return true;
    };
    block_height < before_height
        || (block_height == before_height && tx_index < before_index.unwrap_or(usize::MAX))
}

pub(super) fn transaction_touches_account(
    transaction: &SignedL2Transaction,
    account_id: Hash32,
) -> bool {
    if transaction.from == Some(account_id) {
        return true;
    }
    match &transaction.kind {
        L2TransactionKind::Deposit { recipient, .. } => *recipient == account_id,
        L2TransactionKind::Transfer { to, .. } => *to == account_id,
        L2TransactionKind::Withdraw { .. } => false,
        L2TransactionKind::DeployContract { contract, .. } => *contract == account_id,
        L2TransactionKind::CallContract { contract, .. } => *contract == account_id,
        L2TransactionKind::InternalMessage { from, to, .. } => {
            *from == account_id || *to == account_id
        }
        L2TransactionKind::RotatePublicKey { .. } => false,
    }
}
