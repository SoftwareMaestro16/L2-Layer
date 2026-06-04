use super::*;
use ed25519_dalek::{Signer, SigningKey};
use l2_core::crypto::sha256_bytes;
use l2_core::{L2TransactionKind, SignedL2Transaction};
use rand_core::OsRng;
use tokio::time::sleep;

fn signed_tx(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    kind: L2TransactionKind,
) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        gas_limit: 1_000,
        max_gas_price: 1,
        kind,
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    tx
}

fn service() -> (MempoolService, SigningKey, Hash32) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    (
        MempoolService::new("entropis-testnet", Arc::new(MemoryMempoolStore::default())),
        signing_key,
        account_id,
    )
}

#[tokio::test]
async fn duplicate_tx_hash_is_rejected() {
    let (service, signing_key, account_id) = service();
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"to"),
            asset_id: 0,
            amount: 1,
        },
    );

    assert!(service.submit(tx.clone()).await.is_ok());
    let error = service.submit(tx).await.unwrap_err();
    assert!(matches!(error, MempoolError::DuplicateTx(_)));
}

#[tokio::test]
async fn same_account_nonce_lock_prevents_race() {
    let (service, signing_key, account_id) = service();
    let first = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    let second = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 2,
        },
    );

    assert!(service.submit(first).await.is_ok());
    let error = service.submit(second).await.unwrap_err();
    assert!(matches!(error, MempoolError::NonceLocked { .. }));
}

#[tokio::test]
async fn expired_nonce_lock_is_recoverable() {
    let store = Arc::new(MemoryMempoolStore::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    let tx_hash = tx.tx_hash();

    store
        .enqueue_validated(tx, tx_hash, account_id, 0, Duration::from_millis(5))
        .await
        .unwrap();
    sleep(Duration::from_millis(15)).await;

    let second = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 2,
        },
    );
    assert!(store
        .enqueue_validated(
            second.clone(),
            second.tx_hash(),
            account_id,
            0,
            Duration::from_millis(5),
        )
        .await
        .is_ok());
}

#[tokio::test]
async fn malformed_or_system_tx_is_not_enqueued() {
    let (service, signing_key, account_id) = service();
    let system_tx = SignedL2Transaction::system_deposit(
        "entropis-testnet",
        sha256_bytes(b"deposit"),
        0,
        account_id,
        10,
    );
    assert!(matches!(
        service.submit(system_tx).await.unwrap_err(),
        MempoolError::SystemTxNotAllowed
    ));

    let mut missing_signature = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"to"),
            asset_id: 0,
            amount: 1,
        },
    );
    missing_signature.signature = None;
    assert!(matches!(
        service.submit(missing_signature).await.unwrap_err(),
        MempoolError::MissingSignature
    ));

    assert!(service.pop_batch(10).await.unwrap().is_empty());
}

#[tokio::test]
async fn leader_lock_allows_only_one_owner_until_release_or_expiry() {
    let store = MemoryMempoolStore::default();

    assert!(store
        .acquire_leader_lock("sequencer-a", Duration::from_millis(20))
        .await
        .unwrap());
    assert!(!store
        .acquire_leader_lock("sequencer-b", Duration::from_millis(20))
        .await
        .unwrap());
    assert!(!store.release_leader_lock("sequencer-b").await.unwrap());
    assert!(store.release_leader_lock("sequencer-a").await.unwrap());
    assert!(store
        .acquire_leader_lock("sequencer-b", Duration::from_millis(5))
        .await
        .unwrap());

    sleep(Duration::from_millis(15)).await;
    assert!(store
        .acquire_leader_lock("sequencer-c", Duration::from_millis(5))
        .await
        .unwrap());
}
