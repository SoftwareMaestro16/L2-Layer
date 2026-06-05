use crate::consensus::{batch_data_hash, encode_batch_data};
use crate::crypto::Hash32;
use crate::types::{L2Block, L2BlockHeader, Receipt, SignedL2Transaction, WithdrawalLeaf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchBuildInput {
    pub previous_header: Option<L2BlockHeader>,
    pub prev_state_root: Hash32,
    pub state_root: Hash32,
    pub ordered_transactions: Vec<SignedL2Transaction>,
    pub receipts: Vec<Receipt>,
    pub withdrawals: Vec<WithdrawalLeaf>,
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct BatchDataPayload<'a> {
    pub ordered_transactions: &'a [SignedL2Transaction],
    pub receipts: &'a [Receipt],
}

impl BatchDataPayload<'_> {
    pub fn canonical_bytes(self) -> Vec<u8> {
        canonical_batch_data_bytes(self.ordered_transactions, self.receipts)
    }

    pub fn canonical_hash(self) -> Hash32 {
        canonical_batch_data_hash(self.ordered_transactions, self.receipts)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BatchBuilder;

impl BatchBuilder {
    pub fn build(input: BatchBuildInput) -> Result<L2Block, BatchBuildError> {
        if input.ordered_transactions.len() != input.receipts.len() {
            return Err(BatchBuildError::ReceiptCountMismatch {
                transactions: input.ordered_transactions.len(),
                receipts: input.receipts.len(),
            });
        }

        let (height, prev_block_hash) = match input.previous_header {
            Some(previous_header) => {
                if previous_header.state_root != input.prev_state_root {
                    return Err(BatchBuildError::PrevStateRootMismatch {
                        expected: previous_header.state_root,
                        actual: input.prev_state_root,
                    });
                }
                let height = previous_header.height.checked_add(1).ok_or(
                    BatchBuildError::HeightOverflow {
                        previous_height: previous_header.height,
                    },
                )?;
                (height, previous_header.block_hash())
            }
            None => (0, Hash32::ZERO),
        };

        let data_hash = BatchDataPayload {
            ordered_transactions: &input.ordered_transactions,
            receipts: &input.receipts,
        }
        .canonical_hash();

        L2Block::try_new(
            height,
            prev_block_hash,
            input.prev_state_root,
            input.state_root,
            input.ordered_transactions,
            input.receipts,
            input.withdrawals,
            data_hash,
            input.timestamp,
        )
        .map_err(|error| BatchBuildError::InvalidWithdrawal {
            reason: error.rejection_reason(),
        })
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum BatchBuildError {
    #[error("transaction count {transactions} does not match receipt count {receipts}")]
    ReceiptCountMismatch {
        transactions: usize,
        receipts: usize,
    },
    #[error("previous state root mismatch: expected {expected}, got {actual}")]
    PrevStateRootMismatch { expected: Hash32, actual: Hash32 },
    #[error("previous height {previous_height} cannot be incremented")]
    HeightOverflow { previous_height: u64 },
    #[error("invalid withdrawal release fields: {reason}")]
    InvalidWithdrawal { reason: &'static str },
}

pub fn canonical_batch_data_hash(txs: &[SignedL2Transaction], receipts: &[Receipt]) -> Hash32 {
    batch_data_hash(txs, receipts)
}

pub fn canonical_batch_data_bytes(txs: &[SignedL2Transaction], receipts: &[Receipt]) -> Vec<u8> {
    encode_batch_data(txs, receipts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;
    use crate::merkle::merkle_root;
    use crate::types::{
        L2TransactionKind, ReceiptStatus, L2_NATIVE_GAS_ASSET, L2_TRANSACTION_KIND_VERSION_V1,
        L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
    };

    #[test]
    fn same_input_builds_same_block_hash() {
        let tx = deposit_tx(b"deposit-a");
        let input = input_with(
            vec![tx.clone()],
            vec![Receipt::applied(tx.tx_hash(), 0, None)],
        );

        let first = BatchBuilder::build(input.clone()).expect("first block");
        let second = BatchBuilder::build(input).expect("second block");

        assert_eq!(first, second);
        assert_eq!(first.header.block_hash(), second.header.block_hash());
    }

    #[test]
    fn mismatched_transaction_and_receipt_counts_are_rejected() {
        let tx = deposit_tx(b"deposit-a");
        let error = BatchBuilder::build(input_with(vec![tx], vec![])).expect_err("mismatch");

        assert_eq!(
            error,
            BatchBuildError::ReceiptCountMismatch {
                transactions: 1,
                receipts: 0
            }
        );
    }

    #[test]
    fn previous_state_root_mismatch_is_rejected() {
        let tx = deposit_tx(b"deposit-a");
        let first = BatchBuilder::build(input_with(
            vec![tx.clone()],
            vec![Receipt::applied(tx.tx_hash(), 0, None)],
        ))
        .expect("first block");
        let next_tx = deposit_tx(b"deposit-b");
        let mut next_input = input_with(
            vec![next_tx.clone()],
            vec![Receipt::applied(next_tx.tx_hash(), 0, None)],
        );
        next_input.previous_header = Some(first.header.clone());
        next_input.prev_state_root = sha256_bytes(b"wrong-prev-state");

        let error = BatchBuilder::build(next_input).expect_err("bad previous root");

        assert_eq!(
            error,
            BatchBuildError::PrevStateRootMismatch {
                expected: first.header.state_root,
                actual: sha256_bytes(b"wrong-prev-state")
            }
        );
    }

    #[test]
    fn empty_batch_has_empty_merkle_roots_and_deterministic_data_hash() {
        let block = BatchBuilder::build(input_with(vec![], vec![])).expect("empty block");

        assert_eq!(block.header.tx_root, Hash32::ZERO);
        assert_eq!(block.header.receipt_root, Hash32::ZERO);
        assert_eq!(block.header.withdrawal_root, Hash32::ZERO);
        assert_eq!(block.header.data_hash, canonical_batch_data_hash(&[], &[]));
    }

    #[test]
    fn transaction_order_changes_tx_root_and_block_hash() {
        let first = deposit_tx(b"deposit-a");
        let second = deposit_tx(b"deposit-b");
        let first_receipt = Receipt::applied(first.tx_hash(), 0, None);
        let second_receipt = Receipt::applied(second.tx_hash(), 0, None);

        let canonical = BatchBuilder::build(input_with(
            vec![first.clone(), second.clone()],
            vec![first_receipt.clone(), second_receipt.clone()],
        ))
        .expect("canonical block");
        let reordered = BatchBuilder::build(input_with(
            vec![second, first],
            vec![second_receipt, first_receipt],
        ))
        .expect("reordered block");

        assert_ne!(canonical.header.tx_root, reordered.header.tx_root);
        assert_ne!(canonical.header.block_hash(), reordered.header.block_hash());
    }

    #[test]
    fn rejected_transaction_receipt_is_merkleized_predictably() {
        let tx = deposit_tx(b"deposit-a");
        let receipt = Receipt::rejected(tx.tx_hash(), "bad_nonce");
        let block = BatchBuilder::build(input_with(vec![tx], vec![receipt.clone()]))
            .expect("rejected block");

        assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
        assert_eq!(
            block.header.receipt_root,
            merkle_root(&[receipt.leaf_hash()])
        );
    }

    #[test]
    fn previous_header_drives_next_height_and_previous_block_hash() {
        let tx = deposit_tx(b"deposit-a");
        let first = BatchBuilder::build(input_with(
            vec![tx.clone()],
            vec![Receipt::applied(tx.tx_hash(), 0, None)],
        ))
        .expect("first block");
        let next_tx = deposit_tx(b"deposit-b");
        let mut next_input = input_with(
            vec![next_tx.clone()],
            vec![Receipt::applied(next_tx.tx_hash(), 0, None)],
        );
        next_input.previous_header = Some(first.header.clone());
        next_input.prev_state_root = first.header.state_root;

        let second = BatchBuilder::build(next_input).expect("second block");

        assert_eq!(second.header.height, 1);
        assert_eq!(second.header.prev_block_hash, first.header.block_hash());
    }

    #[test]
    fn previous_height_overflow_is_rejected_without_panic() {
        let tx = deposit_tx(b"deposit-a");
        let first = BatchBuilder::build(input_with(
            vec![tx.clone()],
            vec![Receipt::applied(tx.tx_hash(), 0, None)],
        ))
        .expect("first block");
        let mut saturated_header = first.header.clone();
        saturated_header.height = u64::MAX;
        let next_tx = deposit_tx(b"deposit-b");
        let mut next_input = input_with(
            vec![next_tx.clone()],
            vec![Receipt::applied(next_tx.tx_hash(), 0, None)],
        );
        next_input.previous_header = Some(saturated_header);
        next_input.prev_state_root = first.header.state_root;

        let error = BatchBuilder::build(next_input).expect_err("height overflow");

        assert_eq!(
            error,
            BatchBuildError::HeightOverflow {
                previous_height: u64::MAX
            }
        );
    }

    #[test]
    fn invalid_withdrawal_release_fields_are_rejected_without_panic() {
        let mut input = input_with(vec![], vec![]);
        input.withdrawals.push(WithdrawalLeaf::new(
            sha256_bytes(b"withdraw"),
            L2_NATIVE_GAS_ASSET,
            1,
            sha256_bytes(b"sender"),
            "not-a-ton-address".to_owned(),
        ));

        let error = BatchBuilder::build(input).expect_err("invalid withdrawal");

        assert_eq!(
            error,
            BatchBuildError::InvalidWithdrawal {
                reason: "bad_l1_recipient"
            }
        );
    }

    fn input_with(
        ordered_transactions: Vec<SignedL2Transaction>,
        receipts: Vec<Receipt>,
    ) -> BatchBuildInput {
        BatchBuildInput {
            previous_header: None,
            prev_state_root: Hash32::ZERO,
            state_root: sha256_bytes(b"state-root"),
            ordered_transactions,
            receipts,
            withdrawals: vec![],
            timestamp: 100,
        }
    }

    fn deposit_tx(seed: &[u8]) -> SignedL2Transaction {
        SignedL2Transaction {
            tx_version: L2_TX_VERSION_V2,
            domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
            chain_id: "ton-l2-devnet".to_owned(),
            from: None,
            nonce: 0,
            valid_until_block: u64::MAX,
            gas_limit: 0,
            max_gas_price: 0,
            fee_asset_id: L2_NATIVE_GAS_ASSET,
            memo_hash: None,
            transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
            kind: L2TransactionKind::Deposit {
                deposit_id: sha256_bytes(seed),
                asset_id: L2_NATIVE_GAS_ASSET,
                recipient: sha256_bytes(b"recipient"),
                amount: 10,
            },
            public_key: None,
            signature: None,
        }
    }
}
