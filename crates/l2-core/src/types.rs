use crate::consensus;
use crate::crypto::Hash32;
use crate::merkle::{merkle_root, MerkleProof};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const L2_NATIVE_GAS_ASSET: u32 = 0;
pub const L2_TX_VERSION_V2: u16 = 2;
pub const L2_TRANSACTION_KIND_VERSION_V1: u16 = 1;
pub const L2_TX_DOMAIN_SEPARATOR: &str = "entropis.l2.tx.v2";
pub const MAX_RECEIPT_EVENTS: usize = 16;
pub const MAX_RECEIPT_EVENT_BYTES: usize = 512;

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
    InternalMessage {
        message_id: Hash32,
        from: Hash32,
        to: Hash32,
        #[serde(with = "serde_u128_string")]
        value: u128,
        body_boc_base64: String,
        bounce: bool,
        bounced: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedL2Transaction {
    #[serde(default = "default_tx_version")]
    pub tx_version: u16,
    #[serde(default = "default_tx_domain_separator")]
    pub domain_separator: String,
    pub chain_id: String,
    pub from: Option<Hash32>,
    pub nonce: u64,
    #[serde(default = "default_valid_until_block")]
    pub valid_until_block: u64,
    pub gas_limit: u64,
    #[serde(with = "serde_u128_string")]
    pub max_gas_price: u128,
    #[serde(default)]
    pub fee_asset_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo_hash: Option<Hash32>,
    #[serde(default = "default_transaction_kind_version")]
    pub transaction_kind_version: u16,
    pub kind: L2TransactionKind,
    pub public_key: Option<String>,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnsignedL2Transaction {
    #[serde(default = "default_tx_version")]
    pub tx_version: u16,
    #[serde(default = "default_tx_domain_separator")]
    pub domain_separator: String,
    pub chain_id: String,
    pub from: Option<Hash32>,
    pub nonce: u64,
    #[serde(default = "default_valid_until_block")]
    pub valid_until_block: u64,
    pub gas_limit: u64,
    #[serde(with = "serde_u128_string")]
    pub max_gas_price: u128,
    #[serde(default)]
    pub fee_asset_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo_hash: Option<Hash32>,
    #[serde(default = "default_transaction_kind_version")]
    pub transaction_kind_version: u16,
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
            tx_version: L2_TX_VERSION_V2,
            domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
            chain_id: chain_id.into(),
            from: None,
            nonce: 0,
            valid_until_block: u64::MAX,
            gas_limit: 0,
            max_gas_price: 0,
            fee_asset_id: L2_NATIVE_GAS_ASSET,
            memo_hash: None,
            transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
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

    #[allow(clippy::too_many_arguments)]
    pub fn system_internal_message(
        chain_id: impl Into<String>,
        message_id: Hash32,
        from: Hash32,
        to: Hash32,
        value: u128,
        body_boc: Vec<u8>,
        bounce: bool,
        bounced: bool,
        gas_limit: u64,
    ) -> Self {
        Self {
            tx_version: L2_TX_VERSION_V2,
            domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
            chain_id: chain_id.into(),
            from: None,
            nonce: 0,
            valid_until_block: u64::MAX,
            gas_limit,
            max_gas_price: 0,
            fee_asset_id: L2_NATIVE_GAS_ASSET,
            memo_hash: None,
            transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
            kind: L2TransactionKind::InternalMessage {
                message_id,
                from,
                to,
                value,
                body_boc_base64: BASE64_STANDARD.encode(body_boc),
                bounce,
                bounced,
            },
            public_key: None,
            signature: None,
        }
    }

    pub fn unsigned(&self) -> UnsignedL2Transaction {
        UnsignedL2Transaction {
            tx_version: self.tx_version,
            domain_separator: self.domain_separator.clone(),
            chain_id: self.chain_id.clone(),
            from: self.from,
            nonce: self.nonce,
            valid_until_block: self.valid_until_block,
            gas_limit: self.gas_limit,
            max_gas_price: self.max_gas_price,
            fee_asset_id: self.fee_asset_id,
            memo_hash: self.memo_hash,
            transaction_kind_version: self.transaction_kind_version,
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
        matches!(
            self.kind,
            L2TransactionKind::Deposit { .. } | L2TransactionKind::InternalMessage { .. }
        )
    }
}

pub fn default_tx_version() -> u16 {
    L2_TX_VERSION_V2
}

pub fn default_tx_domain_separator() -> String {
    L2_TX_DOMAIN_SEPARATOR.to_owned()
}

pub fn default_valid_until_block() -> u64 {
    u64::MAX
}

pub fn default_transaction_kind_version() -> u16 {
    L2_TRANSACTION_KIND_VERSION_V1
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
pub enum L2Event {
    ContractDeployed {
        contract: Hash32,
        deployer: Hash32,
        code_hash: Hash32,
        data_hash: Hash32,
    },
    ContractCalled {
        contract: Hash32,
        caller: Hash32,
        body_hash: Hash32,
    },
    WithdrawalCreated {
        withdrawal_id: Hash32,
        asset_id: u32,
        #[serde(with = "serde_u128_string")]
        amount: u128,
        l2_sender: Hash32,
        l1_recipient: String,
    },
    FeeDistributed {
        asset_id: u32,
        #[serde(with = "serde_u128_string")]
        total_amount: u128,
        #[serde(with = "serde_u128_string")]
        sequencer_amount: u128,
        #[serde(with = "serde_u128_string")]
        operator_amount: u128,
        #[serde(with = "serde_u128_string")]
        treasury_amount: u128,
        sequencer_reward_account: Hash32,
        operator_fee_account: Hash32,
        treasury_fee_account: Hash32,
    },
}

impl L2Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ContractDeployed { .. } => "contract_deployed",
            Self::ContractCalled { .. } => "contract_called",
            Self::WithdrawalCreated { .. } => "withdrawal_created",
            Self::FeeDistributed { .. } => "fee_distributed",
        }
    }

    pub fn encoded_size_estimate(&self) -> usize {
        match self {
            Self::ContractDeployed { .. } => 1 + (32 * 4),
            Self::ContractCalled { .. } => 1 + (32 * 3),
            Self::WithdrawalCreated { l1_recipient, .. } => {
                1 + 32 + 4 + 16 + 32 + 4 + l1_recipient.len()
            }
            Self::FeeDistributed { .. } => 1 + 4 + (16 * 4) + (32 * 3),
        }
    }

    pub fn contract(&self) -> Option<Hash32> {
        match self {
            Self::ContractDeployed { contract, .. } | Self::ContractCalled { contract, .. } => {
                Some(*contract)
            }
            Self::WithdrawalCreated { .. } | Self::FeeDistributed { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub tx_hash: Hash32,
    pub status: ReceiptStatus,
    #[serde(with = "serde_u128_string")]
    pub gas_charged: u128,
    pub reason: Option<String>,
    pub withdrawal_id: Option<Hash32>,
    #[serde(default)]
    pub events: Vec<L2Event>,
}

impl Receipt {
    pub fn applied(tx_hash: Hash32, gas_charged: u128, withdrawal_id: Option<Hash32>) -> Self {
        Self {
            tx_hash,
            status: ReceiptStatus::Applied,
            gas_charged,
            reason: None,
            withdrawal_id,
            events: vec![],
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
            events: vec![],
        }
    }

    pub fn with_events(mut self, events: Vec<L2Event>) -> Self {
        debug_assert!(validate_receipt_events(&events).is_ok());
        self.events = events;
        self
    }

    pub fn validate_events(&self) -> Result<(), ReceiptEventError> {
        validate_receipt_events(&self.events)
    }

    pub fn leaf_hash(&self) -> Hash32 {
        consensus::receipt_leaf_hash(self)
    }
}

#[derive(Debug, Error)]
pub enum ReceiptEventError {
    #[error("receipt has {count} events, max {max}")]
    TooManyEvents { count: usize, max: usize },
    #[error("receipt event {kind} is {bytes} bytes, max {max}")]
    EventTooLarge {
        kind: &'static str,
        bytes: usize,
        max: usize,
    },
}

impl ReceiptEventError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::TooManyEvents { .. } => "too_many_receipt_events",
            Self::EventTooLarge { .. } => "receipt_event_too_large",
        }
    }
}

pub fn validate_receipt_events(events: &[L2Event]) -> Result<(), ReceiptEventError> {
    if events.len() > MAX_RECEIPT_EVENTS {
        return Err(ReceiptEventError::TooManyEvents {
            count: events.len(),
            max: MAX_RECEIPT_EVENTS,
        });
    }
    for event in events {
        let bytes = event.encoded_size_estimate();
        if bytes > MAX_RECEIPT_EVENT_BYTES {
            return Err(ReceiptEventError::EventTooLarge {
                kind: event.kind(),
                bytes,
                max: MAX_RECEIPT_EVENT_BYTES,
            });
        }
    }
    Ok(())
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
