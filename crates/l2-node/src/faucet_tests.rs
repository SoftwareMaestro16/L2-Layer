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

#[tokio::test]
async fn faucet_claim_id_reports_duplicate_account_grants() {
    let service = EntFaucetService::from_config(&config()).unwrap();
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let account_id = sha256_bytes(b"repeat-account");

    let first = service
        .grant_claim(&storage, sha256_bytes(b"claim-1"), account_id, None)
        .await
        .unwrap();
    let second = service
        .grant_claim(&storage, sha256_bytes(b"claim-2"), account_id, None)
        .await
        .unwrap();

    assert!(first.response.granted);
    assert_eq!(first.status, EntFaucetBatchClaimStatus::Granted);
    assert!(!second.response.granted);
    assert_eq!(second.status, EntFaucetBatchClaimStatus::DuplicateAccount);
}

#[tokio::test]
async fn faucet_claim_id_is_idempotent() {
    let service = EntFaucetService::from_config(&config()).unwrap();
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let account_id = sha256_bytes(b"account");
    let claim_id = sha256_bytes(b"claim");

    let first = service
        .grant_claim(&storage, claim_id, account_id, None)
        .await
        .unwrap();
    let duplicate = service
        .grant_claim(&storage, claim_id, account_id, None)
        .await
        .unwrap();

    assert!(first.response.granted);
    assert!(!duplicate.response.granted);
    assert_eq!(first.response.deposit_id, duplicate.response.deposit_id);
    assert_eq!(duplicate.status, EntFaucetBatchClaimStatus::DuplicateClaim);
}

#[tokio::test]
async fn faucet_claim_accepts_bounded_custom_amount() {
    let service = EntFaucetService::from_config(&config()).unwrap();
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let account_id = sha256_bytes(b"small-account");

    let grant = service
        .grant_claim(
            &storage,
            sha256_bytes(b"small-claim"),
            account_id,
            Some(100),
        )
        .await
        .unwrap();

    assert_eq!(grant.response.amount_ent, 100);
    assert_eq!(grant.response.amount_base_units, 100_000_000_000);
    assert!(matches!(
        service
            .grant_claim(
                &storage,
                sha256_bytes(b"too-high-claim"),
                sha256_bytes(b"too-high-account"),
                Some(1_001),
            )
            .await
            .unwrap_err(),
        FaucetError::AmountTooHigh
    ));
}
