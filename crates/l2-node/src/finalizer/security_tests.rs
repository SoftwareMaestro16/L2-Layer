use super::*;
use crate::storage::{BatchCommitStatus, DynStorage, InMemoryStorage};
use async_trait::async_trait;
use l2_core::{crypto::sha256_bytes, Hash32, L2Block};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn finalization_confirmation_failure_records_safe_operator_error() {
    let storage = storage_with_submitted_finalization().await;
    let signer = MockFinalizeSigner::default();
    let provider = MockProvider::confirm_error("secret-finalizer-provider-token");
    let finalizer =
        BatchFinalizer::new(config(), storage.clone(), signer.clone(), provider.clone());

    let error = finalizer
        .finalize_once()
        .await
        .expect_err("confirm failure should propagate");

    assert!(matches!(error, RelayerError::Provider(_)));
    assert!(signer.requests().await.is_empty());
    assert_eq!(provider.sent_count().await, 0);
    let record = storage.get_batch_finalization(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchFinalizationStatus::Submitted);
    assert_eq!(record.attempts, 1);
    assert_eq!(
        record.last_error.as_deref(),
        Some("ton provider finalization confirm failed")
    );
    assert!(!record
        .last_error
        .unwrap()
        .contains("secret-finalizer-provider-token"));
}

#[derive(Clone, Default)]
struct MockFinalizeSigner {
    requests: Arc<Mutex<Vec<FinalizeBatchSignRequest>>>,
}

impl MockFinalizeSigner {
    async fn requests(&self) -> Vec<FinalizeBatchSignRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl FinalizeBatchSigner for MockFinalizeSigner {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<crate::signer::SignedFinalizeBatch, crate::signer::SignerClientError> {
        self.requests.lock().await.push(request);
        Ok(crate::signer::SignedFinalizeBatch {
            boc_base64: "te6ccgEBAQEA".to_owned(),
            signer_address: "EQsequencer".to_owned(),
            valid_until: unix_time() + 300,
        })
    }
}

#[derive(Clone, Default)]
struct MockProvider {
    confirm_error: Option<String>,
    sent: Arc<Mutex<Vec<crate::signer::SignedCommitBatch>>>,
}

impl MockProvider {
    fn confirm_error(reason: &str) -> Self {
        Self {
            confirm_error: Some(reason.to_owned()),
            sent: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn sent_count(&self) -> usize {
        self.sent.lock().await.len()
    }
}

#[async_trait]
impl TonCommitProvider for MockProvider {
    async fn send_signed_boc(
        &self,
        signed: &crate::signer::SignedCommitBatch,
    ) -> Result<crate::relayer::TonSubmitResult, RelayerError> {
        self.sent.lock().await.push(signed.clone());
        Ok(crate::relayer::TonSubmitResult {
            message_hash: hash(0x44),
            message_hash_norm: hash(0x45),
        })
    }

    async fn message_confirmed(&self, _message_hash: Hash32) -> Result<bool, RelayerError> {
        if let Some(reason) = &self.confirm_error {
            return Err(RelayerError::Provider(reason.clone()));
        }
        Ok(true)
    }
}

async fn storage_with_submitted_finalization() -> DynStorage {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    storage.save_block(block(0)).await.unwrap();
    let mut commit = storage.get_batch_commit(1).await.unwrap().unwrap();
    commit.status = BatchCommitStatus::Confirmed;
    storage.save_batch_commit(commit.clone()).await.unwrap();
    storage
        .save_batch_finalization(BatchFinalizationRecord {
            batch_no: 1,
            block_height: 0,
            status: BatchFinalizationStatus::Submitted,
            attempts: 1,
            finalize_after_unix: 0,
            message_hash: Some(hash(0x44)),
            message_hash_norm: Some(hash(0x45)),
            last_error: None,
        })
        .await
        .unwrap();
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
