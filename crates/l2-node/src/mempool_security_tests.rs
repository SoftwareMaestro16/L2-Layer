use super::*;
use ed25519_dalek::{Signer, SigningKey};
use l2_core::crypto::{derive_account_id, sha256_bytes};
use l2_core::{
    Hash32, L2TransactionKind, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use std::sync::Arc;

fn signed_transfer(signing_key: &SigningKey, from: Hash32) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce: 0,
        valid_until_block: u64::MAX,
        gas_limit: 1_000,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind: L2TransactionKind::Transfer {
            to: sha256_bytes(b"recipient"),
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 1,
        },
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    tx
}

fn service() -> (MempoolService, SigningKey, Hash32) {
    let signing_key = SigningKey::generate(&mut rand_core::OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    (
        MempoolService::new("entropis-testnet", Arc::new(MemoryMempoolStore::default())),
        signing_key,
        account_id,
    )
}

#[tokio::test]
async fn tx_v2_envelope_rejections_are_counted_and_not_enqueued() {
    let (service, signing_key, account_id) = service();
    let base = signed_transfer(&signing_key, account_id);

    let mut unsupported_version = base.clone();
    unsupported_version.tx_version = 1;
    assert!(matches!(
        service.submit(unsupported_version).await.unwrap_err(),
        MempoolError::UnsupportedTxVersion
    ));

    let mut invalid_domain = base.clone();
    invalid_domain.domain_separator = "entropis.l2.tx.v1".to_owned();
    assert!(matches!(
        service.submit(invalid_domain).await.unwrap_err(),
        MempoolError::InvalidDomainSeparator
    ));

    let mut unsupported_kind_version = base.clone();
    unsupported_kind_version.transaction_kind_version = 99;
    assert!(matches!(
        service.submit(unsupported_kind_version).await.unwrap_err(),
        MempoolError::UnsupportedTransactionKindVersion
    ));

    let mut unsupported_fee_asset = base;
    unsupported_fee_asset.fee_asset_id = 7;
    assert!(matches!(
        service.submit(unsupported_fee_asset).await.unwrap_err(),
        MempoolError::UnsupportedFeeAsset { asset_id: 7 }
    ));

    let metrics = service.metrics().await.unwrap();
    assert_eq!(metrics.accepted, 0);
    assert_eq!(metrics.rejected.get("unsupported_tx_version"), Some(&1));
    assert_eq!(metrics.rejected.get("invalid_domain_separator"), Some(&1));
    assert_eq!(
        metrics.rejected.get("unsupported_transaction_kind_version"),
        Some(&1)
    );
    assert_eq!(metrics.rejected.get("unsupported_fee_asset"), Some(&1));
    assert!(service.pop_batch(10).await.unwrap().is_empty());
}
