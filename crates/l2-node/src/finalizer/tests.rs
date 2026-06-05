use super::*;
use crate::storage::{DynStorage, InMemoryStorage};
use l2_core::crypto::sha256_bytes;
use l2_core::{Hash32, L2Block};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn finalizer_waits_for_challenge_window_before_signing() {
    let storage = storage_with_confirmed_commit(block(0)).await;
    let signer = MockFinalizeSigner::ok("EQsequencer");
    let provider = MockProvider::ok(hash(0x44));
    let mut config = config();
    config.challenge_window_sec = 300;
    let finalizer = BatchFinalizer::new(config, storage.clone(), signer.clone(), provider);

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.created_pending, 1);
    assert_eq!(stats.waiting, 1);
    assert!(signer.requests().await.is_empty());
    let record = storage
        .get_batch_finalization(1)
        .await
        .unwrap()
        .expect("finalization");
    assert_eq!(record.status, BatchFinalizationStatus::Pending);
    assert!(record.finalize_after_unix > unix_time());
}

#[tokio::test]
async fn finalizer_submits_after_challenge_window_and_confirms_once() {
    let storage = storage_with_confirmed_commit(block(0)).await;
    storage
        .save_batch_finalization(BatchFinalizationRecord {
            batch_no: 1,
            block_height: 0,
            status: BatchFinalizationStatus::Pending,
            attempts: 0,
            finalize_after_unix: 0,
            message_hash: None,
            message_hash_norm: None,
            last_error: None,
        })
        .await
        .unwrap();
    let signer = MockFinalizeSigner::ok("EQsequencer");
    let provider = MockProvider::ok(hash(0x44));
    let finalizer =
        BatchFinalizer::new(config(), storage.clone(), signer.clone(), provider.clone());

    let submitted = finalizer.finalize_once().await.expect("submit");
    let confirmed = finalizer.finalize_once().await.expect("confirm");

    assert_eq!(submitted.submitted, 1);
    assert_eq!(confirmed.finalized, 1);
    assert_eq!(signer.requests().await.len(), 1);
    assert_eq!(provider.sent_count().await, 1);
    let record = storage.get_batch_finalization(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchFinalizationStatus::Finalized);
    assert_eq!(record.message_hash_norm, Some(hash(0x45)));
}

#[tokio::test]
async fn bad_finalize_signer_sender_is_rejected_before_provider_send() {
    let storage = storage_with_ready_finalization().await;
    let provider = MockProvider::ok(hash(0x44));
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        MockFinalizeSigner::ok("EQattacker"),
        provider.clone(),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.failed, 1);
    assert_eq!(provider.sent_count().await, 0);
    let record = storage.get_batch_finalization(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchFinalizationStatus::Failed);
    assert_eq!(
        record.last_error.as_deref(),
        Some("finalize signer address mismatch")
    );
}

#[tokio::test]
async fn finalizer_rejects_unconfirmed_commit() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    storage.save_block(block(0)).await.unwrap();
    storage
        .save_batch_finalization(BatchFinalizationRecord {
            batch_no: 1,
            block_height: 0,
            status: BatchFinalizationStatus::Pending,
            attempts: 0,
            finalize_after_unix: 0,
            message_hash: None,
            message_hash_norm: None,
            last_error: None,
        })
        .await
        .unwrap();
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        MockFinalizeSigner::ok("EQsequencer"),
        MockProvider::ok(hash(0x44)),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.failed, 1);
    let record = storage.get_batch_finalization(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchFinalizationStatus::Failed);
    assert_eq!(
        record.last_error.as_deref(),
        Some("batch commit not confirmed")
    );
}

#[tokio::test]
async fn failed_finalization_send_retries_until_max_attempts() {
    let storage = storage_with_ready_finalization().await;
    let signer = MockFinalizeSigner::ok("EQsequencer");
    let provider = MockProvider::failing_send();
    let mut config = config();
    config.max_attempts = 2;
    let finalizer = BatchFinalizer::new(config, storage.clone(), signer, provider.clone());

    assert_eq!(finalizer.finalize_once().await.unwrap().failed, 1);
    assert_eq!(finalizer.finalize_once().await.unwrap().failed, 1);
    assert_eq!(finalizer.finalize_once().await.unwrap().considered, 0);

    let record = storage.get_batch_finalization(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchFinalizationStatus::Failed);
    assert_eq!(record.attempts, 2);
    assert_eq!(provider.sent_count().await, 2);
}

#[tokio::test]
#[ignore = "requires a live TON testnet batch past its challenge window"]
async fn live_testnet_finalization_smoke_requires_env() {
    if std::env::var("ENTROPIS_LIVE_BATCH_FINALIZER")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    panic!(
        "set up a confirmed local batch record, L1_FINALIZE_SIGNER_ENDPOINT, and Toncenter testnet credentials before enabling this smoke"
    );
}

#[derive(Clone, Default)]
struct MockFinalizeSigner {
    signer_address: String,
    boc_base64: String,
    valid_until: u64,
    requests: Arc<Mutex<Vec<FinalizeBatchSignRequest>>>,
}

impl MockFinalizeSigner {
    fn ok(signer_address: &str) -> Self {
        Self {
            signer_address: signer_address.to_owned(),
            boc_base64: "te6ccgEBAQEA".to_owned(),
            valid_until: unix_time() + 300,
            requests: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn requests(&self) -> Vec<FinalizeBatchSignRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl FinalizeBatchSigner for MockFinalizeSigner {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<crate::signer::SignedFinalizeBatch, crate::signer::SignerClientError> {
        self.requests.lock().await.push(request);
        Ok(crate::signer::SignedFinalizeBatch {
            boc_base64: self.boc_base64.clone(),
            signer_address: self.signer_address.clone(),
            valid_until: self.valid_until,
        })
    }
}

#[derive(Clone, Default)]
struct MockProvider {
    send_error: bool,
    message_hash: Hash32,
    sent: Arc<Mutex<Vec<crate::signer::SignedCommitBatch>>>,
}

impl MockProvider {
    fn ok(message_hash: Hash32) -> Self {
        Self {
            send_error: false,
            message_hash,
            sent: Arc::new(Mutex::new(vec![])),
        }
    }

    fn failing_send() -> Self {
        Self {
            send_error: true,
            message_hash: hash(0x44),
            sent: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn sent_count(&self) -> usize {
        self.sent.lock().await.len()
    }
}

#[async_trait::async_trait]
impl TonCommitProvider for MockProvider {
    async fn send_signed_boc(
        &self,
        signed: &crate::signer::SignedCommitBatch,
    ) -> Result<crate::relayer::TonSubmitResult, RelayerError> {
        self.sent.lock().await.push(signed.clone());
        if self.send_error {
            return Err(RelayerError::Provider("network".to_owned()));
        }
        Ok(crate::relayer::TonSubmitResult {
            message_hash: self.message_hash,
            message_hash_norm: hash(self.message_hash.as_bytes()[0] + 1),
        })
    }

    async fn message_confirmed(&self, _message_hash: Hash32) -> Result<bool, RelayerError> {
        Ok(true)
    }
}

async fn storage_with_ready_finalization() -> DynStorage {
    let storage = storage_with_confirmed_commit(block(0)).await;
    storage
        .save_batch_finalization(BatchFinalizationRecord {
            batch_no: 1,
            block_height: 0,
            status: BatchFinalizationStatus::Pending,
            attempts: 0,
            finalize_after_unix: 0,
            message_hash: None,
            message_hash_norm: None,
            last_error: None,
        })
        .await
        .unwrap();
    storage
}

async fn storage_with_confirmed_commit(block: L2Block) -> DynStorage {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    storage.save_block(block).await.unwrap();
    let mut record = storage.get_batch_commit(1).await.unwrap().unwrap();
    record.status = BatchCommitStatus::Confirmed;
    storage.save_batch_commit(record).await.unwrap();
    storage
}

fn config() -> BatchFinalizerConfig {
    BatchFinalizerConfig {
        rollup_root_address: "EQroot".to_owned(),
        sequencer_sender_address: "EQsequencer".to_owned(),
        finalize_msg_value_nanoton: 100_000_000,
        challenge_window_sec: 0,
        poll_interval_ms: 5_000,
        retry_backoff_ms: 15_000,
        max_attempts: 8,
    }
}

fn block(height: u64) -> L2Block {
    L2Block::new(
        height,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        l2_core::canonical_batch_data_hash(&[], &[]),
        100,
    )
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
