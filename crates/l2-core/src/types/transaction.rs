use crate::consensus;
use crate::crypto::Hash32;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

pub const L2_NATIVE_GAS_ASSET: u32 = 0;
pub const L2_TX_VERSION_V2: u16 = 2;
pub const L2_TRANSACTION_KIND_VERSION_V1: u16 = 1;
pub const L2_TX_DOMAIN_SEPARATOR: &str = "entropis.l2.tx.v2";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum L2TransactionKind {
    Deposit {
        deposit_id: Hash32,
        asset_id: u32,
        recipient: Hash32,
        #[serde(with = "crate::types::serde_u128_string")]
        amount: u128,
    },
    Transfer {
        to: Hash32,
        asset_id: u32,
        #[serde(with = "crate::types::serde_u128_string")]
        amount: u128,
    },
    RotatePublicKey {
        new_public_key: String,
    },
    Withdraw {
        asset_id: u32,
        #[serde(with = "crate::types::serde_u128_string")]
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
        #[serde(with = "crate::types::serde_u128_string")]
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
    #[serde(with = "crate::types::serde_u128_string")]
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
    #[serde(with = "crate::types::serde_u128_string")]
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
