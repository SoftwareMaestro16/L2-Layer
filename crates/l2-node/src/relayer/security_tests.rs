use super::*;
use crate::da::{DataAvailabilityConfig, DynDa, StorageDaStore};
use crate::storage::{BatchCommitStatus, DynStorage, InMemoryStorage};
use async_trait::async_trait;
use l2_core::{crypto::sha256_bytes, Hash32, L2Block};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn commit_confirmation_failure_records_safe_operator_error() {
    let (storage, da) = storage_with_submitted_block().await;
    let signer = MockSigner::default();
    let provider = MockProvider::confirm_error("secret-provider-token");
    let relayer = BatchRelayer::new(
        config(),
        storage.clone(),
        da,
        signer.clone(),
        provider.clone(),
    );

    let error = relayer
        .relay_once()
        .await
        .expect_err("confirm failure should propagate");

    assert!(matches!(error, RelayerError::Provider(_)));
    assert!(signer.requests().await.is_empty());
    assert_eq!(provider.sent_count().await, 0);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchCommitStatus::Submitted);
    assert_eq!(record.attempts, 1);
    assert_eq!(
        record.last_error.as_deref(),
        Some("ton provider commit confirm failed")
    );
    assert!(!record.last_error.unwrap().contains("secret-provider-token"));
}

#[derive(Clone, Default)]
struct MockSigner {
    requests: Arc<Mutex<Vec<CommitBatchSignRequest>>>,
}

impl MockSigner {
    async fn requests(&self) -> Vec<CommitBatchSignRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl CommitBatchSigner for MockSigner {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, crate::signer::SignerClientError> {
        self.requests.lock().await.push(request);
        Ok(SignedCommitBatch {
            boc_base64: "te6ccgEBAQEA".to_owned(),
            signer_address: "EQsequencer".to_owned(),
            valid_until: unix_time() + 300,
        })
    }
}

#[derive(Clone, Default)]
struct MockProvider {
    confirm_error: Option<String>,
    sent: Arc<Mutex<Vec<SignedCommitBatch>>>,
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
        signed: &SignedCommitBatch,
    ) -> Result<TonSubmitResult, RelayerError> {
        self.sent.lock().await.push(signed.clone());
        Ok(TonSubmitResult {
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

async fn storage_with_submitted_block() -> (DynStorage, DynDa) {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = da_for_storage(storage.clone());
    let block = block(0);
    da.write_batch_payload(&block).await.unwrap();
    storage.save_block(block).await.unwrap();
    let mut record = storage.get_batch_commit(1).await.unwrap().unwrap();
    record.status = BatchCommitStatus::Submitted;
    record.attempts = 1;
    record.message_hash = Some(hash(0x44));
    record.message_hash_norm = Some(hash(0x45));
    storage.save_batch_commit(record).await.unwrap();
    (storage, da)
}

fn da_for_storage(storage: DynStorage) -> DynDa {
    Arc::new(StorageDaStore::new(
        storage,
        DataAvailabilityConfig {
            max_payload_bytes: crate::da::DEFAULT_DA_MAX_PAYLOAD_BYTES,
            public_backend: crate::da::PublicDaBackend::PostgresOnly,
        },
    ))
}

fn config() -> BatchRelayerConfig {
    BatchRelayerConfig {
        rollup_root_address: "EQroot".to_owned(),
        sequencer_sender_address: "EQsequencer".to_owned(),
        commit_msg_value_nanoton: 100_000_000,
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
