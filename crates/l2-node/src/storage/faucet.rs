use l2_core::Hash32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntFaucetClaimStatus {
    Granted,
    DuplicateAccount,
    Failed,
}

impl EntFaucetClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::DuplicateAccount => "duplicate_account",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "granted" => Some(Self::Granted),
            "duplicate_account" => Some(Self::DuplicateAccount),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetClaimRecord {
    pub batch_id: Hash32,
    pub claim_index: u32,
    pub claim_id: String,
    pub account_id: Hash32,
    #[serde(with = "l2_core::serde_u128_string")]
    pub amount_base_units: u128,
    pub deposit_id: Hash32,
    pub status: EntFaucetClaimStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntFaucetClaimSaveResult {
    pub status: EntFaucetClaimSaveStatus,
    pub record: EntFaucetClaimRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntFaucetClaimSaveStatus {
    Granted,
    DuplicateClaim,
    DuplicateAccount,
}

impl EntFaucetClaimSaveStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::DuplicateClaim => "duplicate_claim",
            Self::DuplicateAccount => "duplicate_account",
        }
    }
}
