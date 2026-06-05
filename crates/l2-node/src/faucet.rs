use l2_core::crypto::{hash_domain, Hash32};
use l2_core::{
    l2_raw_address, l2_user_friendly_address, parse_l2_address, DepositEvent, L2_NATIVE_GAS_ASSET,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::NodeConfig;
use crate::storage::{DynStorage, EntFaucetClaimGrantSave, StorageError};

#[derive(Clone, Debug)]
pub struct EntFaucetService {
    default_amount_ent: u128,
    default_amount_base_units: u128,
    max_amount_ent: u128,
    decimals_multiplier: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetRequest {
    pub account_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetBatchRequest {
    pub claims: Vec<EntFaucetBatchClaimRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetBatchClaimRequest {
    pub claim_id: String,
    pub account_id: String,
    pub amount_ent: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetResponse {
    pub account_id: Hash32,
    pub account_raw_address: String,
    pub account_friendly_address: String,
    #[serde(with = "l2_core::serde_u128_string")]
    pub amount_ent: u128,
    #[serde(with = "l2_core::serde_u128_string")]
    pub amount_base_units: u128,
    pub deposit_id: Hash32,
    pub granted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetBatchResponse {
    pub claims: Vec<EntFaucetBatchClaimResponse>,
    pub totals: EntFaucetBatchTotals,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntFaucetBatchClaimStatus {
    Granted,
    DuplicateClaim,
    DuplicateAccount,
    InvalidAccount,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetBatchClaimResponse {
    pub claim_id: Option<Hash32>,
    pub account_id: Option<Hash32>,
    pub account_raw_address: Option<String>,
    pub account_friendly_address: Option<String>,
    #[serde(with = "l2_core::serde_u128_string")]
    pub amount_ent: u128,
    #[serde(with = "l2_core::serde_u128_string")]
    pub amount_base_units: u128,
    pub deposit_id: Option<Hash32>,
    pub status: EntFaucetBatchClaimStatus,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetBatchTotals {
    pub total: usize,
    pub granted: usize,
    pub duplicate_claim: usize,
    pub duplicate_account: usize,
    pub invalid_account: usize,
    pub failed: usize,
}

impl EntFaucetBatchTotals {
    pub fn from_claims(claims: &[EntFaucetBatchClaimResponse]) -> Self {
        let mut totals = Self {
            total: claims.len(),
            ..Self::default()
        };
        for claim in claims {
            match claim.status {
                EntFaucetBatchClaimStatus::Granted => totals.granted += 1,
                EntFaucetBatchClaimStatus::DuplicateClaim => totals.duplicate_claim += 1,
                EntFaucetBatchClaimStatus::DuplicateAccount => totals.duplicate_account += 1,
                EntFaucetBatchClaimStatus::InvalidAccount => totals.invalid_account += 1,
                EntFaucetBatchClaimStatus::Failed => totals.failed += 1,
            }
        }
        totals
    }
}

#[derive(Clone, Debug)]
pub struct EntFaucetGrant {
    pub response: EntFaucetResponse,
    pub deposit: Option<DepositEvent>,
}

#[derive(Clone, Debug)]
pub struct EntFaucetClaimGrant {
    pub response: EntFaucetResponse,
    pub deposit: Option<DepositEvent>,
    pub status: EntFaucetBatchClaimStatus,
}

#[derive(Debug, Error)]
pub enum FaucetError {
    #[error("invalid account id")]
    InvalidAccountId,
    #[error("invalid claim id")]
    InvalidClaimId,
    #[error("reserved zero address")]
    ZeroAccountId,
    #[error("faucet claim id already exists with different data")]
    ClaimConflict,
    #[error("invalid faucet amount")]
    InvalidAmount,
    #[error("faucet amount exceeds configured maximum")]
    AmountTooHigh,
    #[error("ENT faucet amount overflows base units")]
    AmountOverflow,
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
}

impl EntFaucetService {
    pub fn from_config(config: &NodeConfig) -> Result<Self, FaucetError> {
        let multiplier = 10u128
            .checked_pow(u32::from(config.ent_decimals))
            .ok_or(FaucetError::AmountOverflow)?;
        let amount_base_units = config
            .ent_faucet_amount
            .checked_mul(multiplier)
            .ok_or(FaucetError::AmountOverflow)?;
        Ok(Self {
            default_amount_ent: config.ent_faucet_amount,
            default_amount_base_units: amount_base_units,
            max_amount_ent: config.ent_faucet_max_amount,
            decimals_multiplier: multiplier,
        })
    }

    pub async fn grant(
        &self,
        storage: &DynStorage,
        account_id: Hash32,
    ) -> Result<EntFaucetGrant, FaucetError> {
        if account_id == Hash32::ZERO {
            return Err(FaucetError::ZeroAccountId);
        }

        let deposit = self.deposit_event(account_id);
        let inserted = storage
            .save_ent_faucet_grant(account_id, self.default_amount_base_units)
            .await?;
        let deposit_inserted = if inserted {
            storage.save_deposit(deposit.clone()).await?
        } else {
            false
        };

        Ok(EntFaucetGrant {
            response: EntFaucetResponse {
                account_id,
                account_raw_address: l2_raw_address(account_id),
                account_friendly_address: l2_user_friendly_address(account_id),
                amount_ent: self.default_amount_ent,
                amount_base_units: self.default_amount_base_units,
                deposit_id: deposit.deposit_id,
                granted: deposit_inserted,
            },
            deposit: deposit_inserted.then_some(deposit),
        })
    }

    pub async fn grant_claim(
        &self,
        storage: &DynStorage,
        claim_id: Hash32,
        account_id: Hash32,
        amount_ent: Option<u128>,
    ) -> Result<EntFaucetClaimGrant, FaucetError> {
        if claim_id == Hash32::ZERO {
            return Err(FaucetError::InvalidClaimId);
        }
        if account_id == Hash32::ZERO {
            return Err(FaucetError::ZeroAccountId);
        }

        let amount_ent = amount_ent.unwrap_or(self.default_amount_ent);
        let amount_base_units = self.amount_base_units(amount_ent)?;
        let deposit = self.claim_deposit_event(claim_id, account_id, amount_base_units);
        let saved = match storage
            .save_ent_faucet_claim_grant(claim_id, account_id, amount_base_units)
            .await
        {
            Ok(status) => status,
            Err(StorageError::Conflict {
                resource: "ent_faucet_claim",
            }) => return Err(FaucetError::ClaimConflict),
            Err(error) => return Err(FaucetError::Storage(error)),
        };
        let deposit_inserted = if saved == EntFaucetClaimGrantSave::Inserted {
            storage.save_deposit(deposit.clone()).await?
        } else {
            false
        };
        let status = match saved {
            EntFaucetClaimGrantSave::Inserted if deposit_inserted => {
                EntFaucetBatchClaimStatus::Granted
            }
            EntFaucetClaimGrantSave::Inserted => EntFaucetBatchClaimStatus::Failed,
            EntFaucetClaimGrantSave::DuplicateClaim => EntFaucetBatchClaimStatus::DuplicateClaim,
            EntFaucetClaimGrantSave::DuplicateAccount => {
                EntFaucetBatchClaimStatus::DuplicateAccount
            }
        };

        Ok(EntFaucetClaimGrant {
            response: EntFaucetResponse {
                account_id,
                account_raw_address: l2_raw_address(account_id),
                account_friendly_address: l2_user_friendly_address(account_id),
                amount_ent,
                amount_base_units,
                deposit_id: deposit.deposit_id,
                granted: deposit_inserted,
            },
            deposit: deposit_inserted.then_some(deposit),
            status,
        })
    }

    pub fn parse_account_id(value: &str) -> Result<Hash32, FaucetError> {
        parse_l2_address(value).map_err(|_| FaucetError::InvalidAccountId)
    }

    pub fn parse_claim_id(value: &str) -> Result<Hash32, FaucetError> {
        let value = value.strip_prefix("0x").unwrap_or(value);
        Hash32::from_hex(value).map_err(|_| FaucetError::InvalidClaimId)
    }

    pub fn default_amount_ent(&self) -> u128 {
        self.default_amount_ent
    }

    fn deposit_event(&self, account_id: Hash32) -> DepositEvent {
        let amount_bytes = self.default_amount_base_units.to_be_bytes();
        let deposit_id = hash_domain(
            "entropis.faucet.deposit.v1",
            &[account_id.as_bytes(), &amount_bytes],
        );
        DepositEvent {
            deposit_id,
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: account_id,
            amount: self.default_amount_base_units,
            l1_tx_hash: hash_domain("entropis.faucet.synthetic-l1.v1", &[deposit_id.as_bytes()]),
            l1_lt: 1,
        }
    }

    fn amount_base_units(&self, amount_ent: u128) -> Result<u128, FaucetError> {
        if amount_ent == 0 {
            return Err(FaucetError::InvalidAmount);
        }
        if amount_ent > self.max_amount_ent {
            return Err(FaucetError::AmountTooHigh);
        }
        amount_ent
            .checked_mul(self.decimals_multiplier)
            .ok_or(FaucetError::AmountOverflow)
    }

    fn claim_deposit_event(
        &self,
        claim_id: Hash32,
        account_id: Hash32,
        amount_base_units: u128,
    ) -> DepositEvent {
        let amount_bytes = amount_base_units.to_be_bytes();
        let deposit_id = hash_domain(
            "entropis.faucet.claim.deposit.v1",
            &[claim_id.as_bytes(), account_id.as_bytes(), &amount_bytes],
        );
        DepositEvent {
            deposit_id,
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: account_id,
            amount: amount_base_units,
            l1_tx_hash: hash_domain(
                "entropis.faucet.claim.synthetic-l1.v1",
                &[deposit_id.as_bytes()],
            ),
            l1_lt: 1,
        }
    }
}

#[cfg(test)]
#[path = "faucet_tests.rs"]
mod tests;
