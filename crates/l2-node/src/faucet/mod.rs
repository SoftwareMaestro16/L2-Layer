use l2_core::crypto::{hash_domain, Hash32};
use l2_core::{
    l2_raw_address, l2_user_friendly_address, parse_l2_address, DepositEvent, L2_NATIVE_GAS_ASSET,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::NodeConfig;
use crate::storage::{DynStorage, StorageError};

#[derive(Clone, Debug)]
pub struct EntFaucetService {
    amount_ent: u128,
    amount_base_units: u128,
    decimals: u8,
}

pub const MAX_ENT_FAUCET_BATCH_CLAIMS: usize = 100;
const MAX_ENT_FAUCET_CLAIM_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntFaucetRequest {
    pub account_id: String,
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

#[derive(Clone, Debug)]
pub struct EntFaucetGrant {
    pub response: EntFaucetResponse,
    pub deposit: Option<DepositEvent>,
}

#[derive(Debug, Error)]
pub enum FaucetError {
    #[error("invalid account id")]
    InvalidAccountId,
    #[error("invalid claim id")]
    InvalidClaimId,
    #[error("reserved zero address")]
    ZeroAccountId,
    #[error("invalid faucet amount")]
    InvalidAmount,
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
            amount_ent: config.ent_faucet_amount,
            amount_base_units,
            decimals: config.ent_decimals,
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
            .save_ent_faucet_grant(account_id, self.amount_base_units)
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
                amount_ent: self.amount_ent,
                amount_base_units: self.amount_base_units,
                deposit_id: deposit.deposit_id,
                granted: deposit_inserted,
            },
            deposit: deposit_inserted.then_some(deposit),
        })
    }

    pub fn parse_account_id(value: &str) -> Result<Hash32, FaucetError> {
        parse_l2_address(value).map_err(|_| FaucetError::InvalidAccountId)
    }

    pub fn default_amount_ent(&self) -> u128 {
        self.amount_ent
    }

    pub fn default_amount_base_units(&self) -> u128 {
        self.amount_base_units
    }

    pub fn amount_ent_to_base_units(&self, amount_ent: u128) -> Result<u128, FaucetError> {
        if amount_ent == 0 || amount_ent > self.amount_ent {
            return Err(FaucetError::InvalidAmount);
        }
        let multiplier = 10u128
            .checked_pow(u32::from(self.decimals))
            .ok_or(FaucetError::AmountOverflow)?;
        amount_ent
            .checked_mul(multiplier)
            .ok_or(FaucetError::AmountOverflow)
    }

    pub fn amount_base_units_to_ent(&self, amount_base_units: u128) -> Result<u128, FaucetError> {
        let multiplier = 10u128
            .checked_pow(u32::from(self.decimals))
            .ok_or(FaucetError::AmountOverflow)?;
        Ok(amount_base_units / multiplier)
    }

    pub fn validate_claim_id(value: &str) -> Result<(), FaucetError> {
        if value.is_empty() || value.len() > MAX_ENT_FAUCET_CLAIM_ID_BYTES {
            return Err(FaucetError::InvalidClaimId);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        {
            return Err(FaucetError::InvalidClaimId);
        }
        Ok(())
    }

    pub fn batch_id<'a>(claim_ids: impl IntoIterator<Item = &'a str>) -> Hash32 {
        let joined = claim_ids.into_iter().fold(Vec::new(), |mut acc, claim_id| {
            if !acc.is_empty() {
                acc.push(0);
            }
            acc.extend_from_slice(claim_id.as_bytes());
            acc
        });
        hash_domain("entropis.faucet.batch.v1", &[&joined])
    }

    pub fn batch_deposit_event(
        &self,
        claim_id: &str,
        account_id: Hash32,
        amount_base_units: u128,
    ) -> DepositEvent {
        let amount_bytes = amount_base_units.to_be_bytes();
        let deposit_id = hash_domain(
            "entropis.faucet.batch.deposit.v1",
            &[claim_id.as_bytes(), account_id.as_bytes(), &amount_bytes],
        );
        DepositEvent {
            deposit_id,
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: account_id,
            amount: amount_base_units,
            l1_tx_hash: hash_domain(
                "entropis.faucet.batch.synthetic-l1.v1",
                &[deposit_id.as_bytes()],
            ),
            l1_lt: 1,
        }
    }

    fn deposit_event(&self, account_id: Hash32) -> DepositEvent {
        let amount_bytes = self.amount_base_units.to_be_bytes();
        let deposit_id = hash_domain(
            "entropis.faucet.deposit.v1",
            &[account_id.as_bytes(), &amount_bytes],
        );
        DepositEvent {
            deposit_id,
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: account_id,
            amount: self.amount_base_units,
            l1_tx_hash: hash_domain("entropis.faucet.synthetic-l1.v1", &[deposit_id.as_bytes()]),
            l1_lt: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;
    use l2_core::crypto::sha256_bytes;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn config() -> NodeConfig {
        let env = BTreeMap::from([
            ("L2_NAME".to_owned(), "Entropis".to_owned()),
            ("L2_CHAIN_ID".to_owned(), "entropis-testnet".to_owned()),
            ("L2_NATIVE_TOKEN_NAME".to_owned(), "Entropis".to_owned()),
            ("L2_NATIVE_TOKEN_SYMBOL".to_owned(), "ENT".to_owned()),
            ("TON_NETWORK".to_owned(), "testnet".to_owned()),
            (
                "TONCENTER_V3_BASE_URL".to_owned(),
                "https://testnet.toncenter.com/api/v3".to_owned(),
            ),
            (
                "TONCENTER_API_KEY".to_owned(),
                "test-api-token-a".to_owned(),
            ),
            (
                "TONAPI_BASE_URL".to_owned(),
                "https://testnet.tonapi.io".to_owned(),
            ),
            ("TONAPI_KEY".to_owned(), "test-api-token-b".to_owned()),
            (
                "DATABASE_URL".to_owned(),
                "postgresql://user:pass@localhost:5432/l2".to_owned(),
            ),
            (
                "REDIS_URL".to_owned(),
                "redis://default:pass@localhost:6379".to_owned(),
            ),
            ("L2_ADMIN_TOKEN".to_owned(), "admin-secret-token".to_owned()),
            ("ENT_DECIMALS".to_owned(), "9".to_owned()),
            ("ENT_LOGO_PATH".to_owned(), "assets/entropis.png".to_owned()),
            ("ENT_FAUCET_REQUIRE_ADMIN".to_owned(), "true".to_owned()),
        ]);
        NodeConfig::from_lookup(|key| env.get(key).cloned()).expect("valid config")
    }

    #[tokio::test]
    async fn faucet_grant_is_idempotent_and_uses_base_units() {
        let service = EntFaucetService::from_config(&config()).unwrap();
        let storage: DynStorage = Arc::new(InMemoryStorage::default());
        let account_id = sha256_bytes(b"account");

        let first = service.grant(&storage, account_id).await.unwrap();
        assert!(first.response.granted);
        assert_eq!(first.response.amount_ent, 1_000);
        assert_eq!(first.response.amount_base_units, 1_000_000_000_000);
        assert!(first.deposit.is_some());

        let duplicate = service.grant(&storage, account_id).await.unwrap();
        assert!(!duplicate.response.granted);
        assert_eq!(duplicate.response.deposit_id, first.response.deposit_id);
        assert!(duplicate.deposit.is_none());
    }

    #[tokio::test]
    async fn faucet_rejects_zero_account() {
        let service = EntFaucetService::from_config(&config()).unwrap();
        let storage: DynStorage = Arc::new(InMemoryStorage::default());

        assert!(matches!(
            service.grant(&storage, Hash32::ZERO).await.unwrap_err(),
            FaucetError::ZeroAccountId
        ));
    }
}
