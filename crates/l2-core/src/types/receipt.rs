use crate::consensus;
use crate::crypto::Hash32;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_RECEIPT_EVENTS: usize = 16;
pub const MAX_RECEIPT_EVENT_BYTES: usize = 512;

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
        #[serde(with = "super::serde_u128_string")]
        amount: u128,
        l2_sender: Hash32,
        l1_recipient: String,
    },
}

impl L2Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ContractDeployed { .. } => "contract_deployed",
            Self::ContractCalled { .. } => "contract_called",
            Self::WithdrawalCreated { .. } => "withdrawal_created",
        }
    }

    pub fn encoded_size_estimate(&self) -> usize {
        match self {
            Self::ContractDeployed { .. } => 1 + (32 * 4),
            Self::ContractCalled { .. } => 1 + (32 * 3),
            Self::WithdrawalCreated { l1_recipient, .. } => {
                1 + 32 + 4 + 16 + 32 + 4 + l1_recipient.len()
            }
        }
    }

    pub fn contract(&self) -> Option<Hash32> {
        match self {
            Self::ContractDeployed { contract, .. } | Self::ContractCalled { contract, .. } => {
                Some(*contract)
            }
            Self::WithdrawalCreated { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub tx_hash: Hash32,
    pub status: ReceiptStatus,
    #[serde(with = "super::serde_u128_string")]
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
