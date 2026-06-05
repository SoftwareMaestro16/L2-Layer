use crate::consensus;
use crate::crypto::Hash32;
use crate::merkle::{merkle_root, MerkleProof};
use crate::types::{Receipt, ReceiptEventError, SignedL2Transaction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DepositEvent {
    pub deposit_id: Hash32,
    pub asset_id: u32,
    pub recipient: Hash32,
    #[serde(with = "crate::types::serde_u128_string")]
    pub amount: u128,
    pub l1_tx_hash: Hash32,
    pub l1_lt: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalLeaf {
    pub withdrawal_id: Hash32,
    pub asset_id: u32,
    #[serde(with = "crate::types::serde_u128_string")]
    pub amount: u128,
    pub l2_sender: Hash32,
    pub l1_recipient: String,
}

impl WithdrawalLeaf {
    pub fn new(
        tx_hash: Hash32,
        asset_id: u32,
        amount: u128,
        l2_sender: Hash32,
        l1_recipient: String,
    ) -> Self {
        let withdrawal_id =
            consensus::withdrawal_id(tx_hash, asset_id, amount, l2_sender, &l1_recipient);
        Self {
            withdrawal_id,
            asset_id,
            amount,
            l2_sender,
            l1_recipient,
        }
    }

    pub fn leaf_hash(&self) -> Hash32 {
        consensus::withdrawal_leaf_hash(self)
    }

    pub fn release_leaf_hash(&self) -> Result<Hash32, crate::withdrawal::WithdrawalProofError> {
        crate::withdrawal::release_leaf_hash(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct L2BlockHeader {
    pub height: u64,
    pub prev_block_hash: Hash32,
    pub prev_state_root: Hash32,
    pub state_root: Hash32,
    pub tx_root: Hash32,
    pub receipt_root: Hash32,
    pub withdrawal_root: Hash32,
    pub data_hash: Hash32,
    pub timestamp: u64,
}

impl L2BlockHeader {
    pub fn block_hash(&self) -> Hash32 {
        consensus::block_header_hash(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct L2Block {
    pub header: L2BlockHeader,
    pub transactions: Vec<SignedL2Transaction>,
    pub receipts: Vec<Receipt>,
    pub withdrawals: Vec<WithdrawalLeaf>,
}

impl L2Block {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        height: u64,
        prev_block_hash: Hash32,
        prev_state_root: Hash32,
        state_root: Hash32,
        transactions: Vec<SignedL2Transaction>,
        receipts: Vec<Receipt>,
        withdrawals: Vec<WithdrawalLeaf>,
        data_hash: Hash32,
        timestamp: u64,
    ) -> Self {
        Self::try_new(
            height,
            prev_block_hash,
            prev_state_root,
            state_root,
            transactions,
            receipts,
            withdrawals,
            data_hash,
            timestamp,
        )
        .expect("block fields must be valid before block construction")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        height: u64,
        prev_block_hash: Hash32,
        prev_state_root: Hash32,
        state_root: Hash32,
        transactions: Vec<SignedL2Transaction>,
        receipts: Vec<Receipt>,
        withdrawals: Vec<WithdrawalLeaf>,
        data_hash: Hash32,
        timestamp: u64,
    ) -> Result<Self, BlockConstructionError> {
        for receipt in &receipts {
            receipt.validate_events()?;
        }
        let tx_hashes = transactions
            .iter()
            .map(SignedL2Transaction::tx_hash)
            .collect::<Vec<_>>();
        let receipt_hashes = receipts.iter().map(Receipt::leaf_hash).collect::<Vec<_>>();
        let withdrawal_root = crate::withdrawal::withdrawal_merkle_root(&withdrawals)?;

        Ok(Self {
            header: L2BlockHeader {
                height,
                prev_block_hash,
                prev_state_root,
                state_root,
                tx_root: merkle_root(&tx_hashes),
                receipt_root: merkle_root(&receipt_hashes),
                withdrawal_root,
                data_hash,
                timestamp,
            },
            transactions,
            receipts,
            withdrawals,
        })
    }

    pub fn withdrawal_proof(&self, withdrawal_id: Hash32) -> Option<WithdrawalProof> {
        let index = self
            .withdrawals
            .iter()
            .position(|leaf| leaf.withdrawal_id == withdrawal_id)?;
        let proof =
            crate::withdrawal::build_withdrawal_merkle_proof(&self.withdrawals, index).ok()??;
        Some(WithdrawalProof {
            block_height: self.header.height,
            withdrawal_root: self.header.withdrawal_root,
            leaf: self.withdrawals[index].clone(),
            proof,
        })
    }
}

#[derive(Debug, Error)]
pub enum BlockConstructionError {
    #[error("invalid withdrawal release fields: {0}")]
    InvalidWithdrawal(#[from] crate::withdrawal::WithdrawalProofError),
    #[error("invalid receipt events: {0}")]
    InvalidReceiptEvents(#[from] ReceiptEventError),
}

impl BlockConstructionError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::InvalidWithdrawal(error) => error.rejection_reason(),
            Self::InvalidReceiptEvents(error) => error.rejection_reason(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalProof {
    pub block_height: u64,
    pub withdrawal_root: Hash32,
    pub leaf: WithdrawalLeaf,
    pub proof: MerkleProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubmitTxResponse {
    pub tx_hash: Hash32,
}
