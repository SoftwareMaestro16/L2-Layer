use crate::consensus;
use crate::crypto::Hash32;
use crate::merkle::{merkle_root, MerkleProof};
use serde::{Deserialize, Serialize};

pub const L2_NATIVE_GAS_ASSET: u32 = 0;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum L2TransactionKind {
    Deposit {
        deposit_id: Hash32,
        asset_id: u32,
        recipient: Hash32,
        #[serde(with = "serde_u128_string")]
        amount: u128,
    },
    Transfer {
        to: Hash32,
        asset_id: u32,
        #[serde(with = "serde_u128_string")]
        amount: u128,
    },
    RotatePublicKey {
        new_public_key: String,
    },
    Withdraw {
        asset_id: u32,
        #[serde(with = "serde_u128_string")]
        amount: u128,
        l1_recipient: String,
    },
    DeployContract {
        contract: Hash32,
        code_boc_base64: String,
        data_boc_base64: String,
    },
    CallContract {
        contract: Hash32,
        body_boc_base64: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedL2Transaction {
    pub chain_id: String,
    pub from: Option<Hash32>,
    pub nonce: u64,
    pub gas_limit: u64,
    #[serde(with = "serde_u128_string")]
    pub max_gas_price: u128,
    pub kind: L2TransactionKind,
    pub public_key: Option<String>,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnsignedL2Transaction {
    pub chain_id: String,
    pub from: Option<Hash32>,
    pub nonce: u64,
    pub gas_limit: u64,
    #[serde(with = "serde_u128_string")]
    pub max_gas_price: u128,
    pub kind: L2TransactionKind,
}

impl SignedL2Transaction {
    pub fn system_deposit(
        chain_id: impl Into<String>,
        deposit_id: Hash32,
        asset_id: u32,
        recipient: Hash32,
        amount: u128,
    ) -> Self {
        Self {
            chain_id: chain_id.into(),
            from: None,
            nonce: 0,
            gas_limit: 0,
            max_gas_price: 0,
            kind: L2TransactionKind::Deposit {
                deposit_id,
                asset_id,
                recipient,
                amount,
            },
            public_key: None,
            signature: None,
        }
    }

    pub fn unsigned(&self) -> UnsignedL2Transaction {
        UnsignedL2Transaction {
            chain_id: self.chain_id.clone(),
            from: self.from,
            nonce: self.nonce,
            gas_limit: self.gas_limit,
            max_gas_price: self.max_gas_price,
            kind: self.kind.clone(),
        }
    }

    pub fn signing_payload(&self) -> Vec<u8> {
        consensus::signing_payload(self)
    }

    pub fn tx_hash(&self) -> Hash32 {
        consensus::transaction_hash(self)
    }

    pub fn is_system(&self) -> bool {
        matches!(self.kind, L2TransactionKind::Deposit { .. })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DepositEvent {
    pub deposit_id: Hash32,
    pub asset_id: u32,
    pub recipient: Hash32,
    #[serde(with = "serde_u128_string")]
    pub amount: u128,
    pub l1_tx_hash: Hash32,
    pub l1_lt: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    Applied,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub tx_hash: Hash32,
    pub status: ReceiptStatus,
    #[serde(with = "serde_u128_string")]
    pub gas_charged: u128,
    pub reason: Option<String>,
    pub withdrawal_id: Option<Hash32>,
}

impl Receipt {
    pub fn applied(tx_hash: Hash32, gas_charged: u128, withdrawal_id: Option<Hash32>) -> Self {
        Self {
            tx_hash,
            status: ReceiptStatus::Applied,
            gas_charged,
            reason: None,
            withdrawal_id,
        }
    }

    pub fn rejected(tx_hash: Hash32, reason: impl Into<String>) -> Self {
        Self::rejected_with_gas(tx_hash, reason, 0)
    }

    pub fn rejected_with_gas(
        tx_hash: Hash32,
        reason: impl Into<String>,
        gas_charged: u128,
    ) -> Self {
        Self {
            tx_hash,
            status: ReceiptStatus::Rejected,
            gas_charged,
            reason: Some(reason.into()),
            withdrawal_id: None,
        }
    }

    pub fn leaf_hash(&self) -> Hash32 {
        consensus::receipt_leaf_hash(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalLeaf {
    pub withdrawal_id: Hash32,
    pub asset_id: u32,
    #[serde(with = "serde_u128_string")]
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
        .expect("withdrawal release fields must be valid before block construction")
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
    ) -> Result<Self, crate::withdrawal::WithdrawalProofError> {
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

pub mod serde_u128_string {
    use serde::de::{Unexpected, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U128Visitor;

        impl<'de> Visitor<'de> for U128Visitor {
            type Value = u128;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a u128 as a JSON string or unsigned integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                value.parse::<u128>().map_err(E::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(value as u128)
            }

            fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(value)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if value < 0 {
                    return Err(E::invalid_value(Unexpected::Signed(value), &self));
                }
                Ok(value as u128)
            }
        }

        deserializer.deserialize_any(U128Visitor)
    }
}
