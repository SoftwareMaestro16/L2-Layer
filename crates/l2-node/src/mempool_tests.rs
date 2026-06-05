use super::*;
use ed25519_dalek::{Signer, SigningKey};
use l2_core::crypto::{derive_account_id, sha256_bytes};
use l2_core::{
    Hash32, L2TransactionKind, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use rand_core::OsRng;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

fn signed_tx(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    kind: L2TransactionKind,
) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit: 1_000,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind,
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    tx
}

fn resign(signing_key: &SigningKey, tx: &mut SignedL2Transaction) {
    tx.signature = None;
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
}

fn service() -> (MempoolService, SigningKey, Hash32) {
    service_with_config(MempoolAdmissionConfig::default())
}

fn service_with_config(config: MempoolAdmissionConfig) -> (MempoolService, SigningKey, Hash32) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    (
        MempoolService::with_config(
            "entropis-testnet",
            config,
            Arc::new(MemoryMempoolStore::default()),
        ),
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
    let config = MempoolAdmissionConfig {
        nonce_lock_ttl: Duration::from_millis(5),
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);
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

    service.submit(tx).await.unwrap();
    sleep(Duration::from_millis(15)).await;
    assert_eq!(service.pop_batch(1).await.unwrap().len(), 1);

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
    assert!(service.submit(second).await.is_ok());
}

#[tokio::test]
async fn pending_nonce_window_rejects_distant_account_nonce() {
    let config = MempoolAdmissionConfig {
        max_account_nonce_window: 2,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);
    let first = signed_tx(
        &signing_key,
        account_id,
        10,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    let distant = signed_tx(
        &signing_key,
        account_id,
        13,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 1,
        },
    );

    service.submit(first).await.unwrap();
    assert!(matches!(
        service.submit(distant).await.unwrap_err(),
        MempoolError::AccountNonceWindowExceeded { .. }
    ));
}

#[tokio::test]
async fn operator_banned_account_is_rejected_before_signature_check() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let config = MempoolAdmissionConfig {
        banned_accounts: [account_id].into_iter().collect(),
        ..MempoolAdmissionConfig::default()
    };
    let service = MempoolService::with_config(
        "entropis-testnet",
        config,
        Arc::new(MemoryMempoolStore::default()),
    );
    let mut tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    tx.signature = Some("00".repeat(64));

    assert!(matches!(
        service.submit(tx).await.unwrap_err(),
        MempoolError::AccountBanned { .. }
    ));
    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.rejected.get("account_banned"), Some(&1));
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
async fn reserved_zero_address_endpoint_is_not_enqueued() {
    let (service, signing_key, account_id) = service();
    let transfer = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: Hash32::ZERO,
            asset_id: 0,
            amount: 1,
        },
    );

    assert!(matches!(
        service.submit(transfer).await.unwrap_err(),
        MempoolError::ReservedZeroAddress
    ));
    assert!(service.pop_batch(10).await.unwrap().is_empty());
    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.rejected.get("reserved_zero_address"), Some(&1));
}

#[tokio::test]
async fn wrong_chain_id_and_bad_signature_are_rejected() {
    let (service, signing_key, account_id) = service();
    let mut wrong_chain = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"to"),
            asset_id: 0,
            amount: 1,
        },
    );
    wrong_chain.chain_id = "wrong-chain".to_owned();
    assert!(matches!(
        service.submit(wrong_chain).await.unwrap_err(),
        MempoolError::WrongChainId
    ));

    let mut bad_sig = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"to"),
            asset_id: 0,
            amount: 1,
        },
    );
    bad_sig.signature = Some("00".repeat(64));
    assert!(matches!(
        service.submit(bad_sig).await.unwrap_err(),
        MempoolError::BadSignature
    ));
}

#[tokio::test]
async fn global_queue_limit_prevents_flooding() {
    let config = MempoolAdmissionConfig {
        max_global_queue: 1,
        ..MempoolAdmissionConfig::default()
    };
    let (service, first_key, first_account) = service_with_config(config);
    let second_key = SigningKey::generate(&mut OsRng);
    let second_account = derive_account_id(&second_key.verifying_key().to_bytes());

    let first = signed_tx(
        &first_key,
        first_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    let second = signed_tx(
        &second_key,
        second_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 1,
        },
    );

    service.submit(first).await.unwrap();
    assert!(matches!(
        service.submit(second).await.unwrap_err(),
        MempoolError::GlobalQueueFull
    ));
}

#[tokio::test]
async fn high_fee_transaction_evicts_lowest_priority_when_global_queue_is_full() {
    let config = MempoolAdmissionConfig {
        max_global_queue: 1,
        ..MempoolAdmissionConfig::default()
    };
    let (service, low_key, low_account) = service_with_config(config);
    let high_key = SigningKey::generate(&mut OsRng);
    let high_account = derive_account_id(&high_key.verifying_key().to_bytes());

    let low = signed_tx(
        &low_key,
        low_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"low"),
            asset_id: 0,
            amount: 1,
        },
    );
    let mut high = signed_tx(
        &high_key,
        high_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"high"),
            asset_id: 0,
            amount: 1,
        },
    );
    high.max_gas_price = 2;
    resign(&high_key, &mut high);
    let high_hash = high.tx_hash();

    service.submit(low).await.unwrap();
    service.submit(high).await.unwrap();
    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.store.evicted, 1);
    let popped = service.pop_batch(1).await.unwrap();
    assert_eq!(popped[0].tx_hash(), high_hash);
}

#[tokio::test]
async fn per_account_queue_limit_prevents_account_flooding() {
    let config = MempoolAdmissionConfig {
        max_account_queue: 1,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);

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
        1,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 1,
        },
    );

    service.submit(first).await.unwrap();
    assert!(matches!(
        service.submit(second).await.unwrap_err(),
        MempoolError::AccountQueueFull { .. }
    ));
}

#[tokio::test]
async fn fair_pop_interleaves_accounts_before_repeating_one_account() {
    let (service, first_key, first_account) = service();
    let second_key = SigningKey::generate(&mut OsRng);
    let second_account = derive_account_id(&second_key.verifying_key().to_bytes());

    for nonce in 0..3 {
        let tx = signed_tx(
            &first_key,
            first_account,
            nonce,
            L2TransactionKind::Transfer {
                to: sha256_bytes(format!("first-{nonce}").as_bytes()),
                asset_id: 0,
                amount: 1,
            },
        );
        service.submit(tx).await.unwrap();
    }
    let second = signed_tx(
        &second_key,
        second_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"second"),
            asset_id: 0,
            amount: 1,
        },
    );
    service.submit(second).await.unwrap();

    let popped = service.pop_batch(2).await.unwrap();
    let accounts = popped.iter().map(|tx| tx.from.unwrap()).collect::<Vec<_>>();
    assert!(accounts.contains(&first_account));
    assert!(accounts.contains(&second_account));
}

#[tokio::test]
async fn bad_signature_spam_consumes_rate_limit() {
    let config = MempoolAdmissionConfig {
        max_account_submissions_per_window: 1,
        rate_limit_window: Duration::from_secs(60),
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);

    let mut bad_sig = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    bad_sig.signature = Some("00".repeat(64));
    assert!(matches!(
        service.submit(bad_sig).await.unwrap_err(),
        MempoolError::BadSignature
    ));

    let valid = signed_tx(
        &signing_key,
        account_id,
        1,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 1,
        },
    );
    assert!(matches!(
        service.submit(valid).await.unwrap_err(),
        MempoolError::RateLimited { .. }
    ));
}

#[tokio::test]
async fn admission_policy_rejects_bad_gas_and_oversized_payloads() {
    let config = MempoolAdmissionConfig {
        max_payload_bytes: 2048,
        max_call_body_boc_base64_bytes: 8,
        max_tx_fee: 10,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);

    let mut zero_gas = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"a"),
            asset_id: 0,
            amount: 1,
        },
    );
    zero_gas.gas_limit = 0;
    zero_gas.signature = None;
    let signature = signing_key.sign(&zero_gas.signing_payload());
    zero_gas.signature = Some(hex::encode(signature.to_bytes()));
    assert!(matches!(
        service.submit(zero_gas).await.unwrap_err(),
        MempoolError::InvalidGasLimit { .. }
    ));

    let high_fee = signed_tx(
        &signing_key,
        account_id,
        1,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"b"),
            asset_id: 0,
            amount: 1,
        },
    );
    assert!(matches!(
        service.submit(high_fee).await.unwrap_err(),
        MempoolError::TxFeeTooHigh { .. }
    ));

    let call_config = MempoolAdmissionConfig {
        max_payload_bytes: 2048,
        max_call_body_boc_base64_bytes: 8,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(call_config);
    let oversized_call = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::CallContract {
            contract: sha256_bytes(b"contract"),
            body_boc_base64: "A".repeat(12),
        },
    );
    assert!(matches!(
        service.submit(oversized_call).await.unwrap_err(),
        MempoolError::PayloadTooLarge { .. } | MempoolError::CallBodyTooLarge { .. }
    ));
}

#[tokio::test]
async fn payload_class_limits_have_distinct_rejection_reasons() {
    let config = MempoolAdmissionConfig {
        max_payload_bytes: 4096,
        max_transfer_payload_bytes: 64,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);
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

    assert!(matches!(
        service.submit(tx).await.unwrap_err(),
        MempoolError::PayloadClassTooLarge { .. }
    ));
    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.rejected.get("transfer_payload_too_large"), Some(&1));
}

#[tokio::test]
async fn admission_policy_rejects_oversized_public_tx_payload() {
    let config = MempoolAdmissionConfig {
        max_payload_bytes: 64,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);
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

    assert!(matches!(
        service.submit(tx).await.unwrap_err(),
        MempoolError::PayloadTooLarge { .. }
    ));
}

#[tokio::test]
async fn malformed_call_body_base64_is_rejected() {
    let config = MempoolAdmissionConfig {
        max_payload_bytes: 2048,
        max_call_body_boc_base64_bytes: 64,
        ..MempoolAdmissionConfig::default()
    };
    let (service, signing_key, account_id) = service_with_config(config);
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::CallContract {
            contract: sha256_bytes(b"contract"),
            body_boc_base64: "***not-base64***".to_owned(),
        },
    );

    assert!(matches!(
        service.submit(tx).await.unwrap_err(),
        MempoolError::BadCallBodyBase64
    ));
}

#[tokio::test]
async fn metrics_track_accepts_rejections_and_queue_depth() {
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
    service.submit(tx.clone()).await.unwrap();
    let _ = service.submit(tx).await.unwrap_err();

    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.accepted, 1);
    assert_eq!(metrics.rejected.get("duplicate_tx"), Some(&1));
    assert_eq!(metrics.store.queued_global, 1);
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
