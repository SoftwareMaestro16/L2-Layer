use super::*;
use crate::relayer::TonSubmitResult;
use crate::storage::{BatchCommitStatus, BatchFinalizationStatus, DynStorage, InMemoryStorage};
use l2_core::{canonical_batch_data_hash, crypto::sha256_bytes, Hash32, L2Block};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn finalizer_waits_until_challenge_window_elapsed() {
    let storage = confirmed_storage().await;
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::with_commitment(1, commitment(100, false));
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        signer.clone(),
        provider,
        clock(150),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.not_ready, 1);
    assert!(signer.requests().await.is_empty());
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.finalization_status, BatchFinalizationStatus::Pending);
    assert_eq!(record.l1_committed_at, Some(100));
    assert_eq!(record.finalization_eligible_at, Some(200));
}

#[tokio::test]
async fn eligible_batch_submits_finalize_and_stores_status() {
    let storage = confirmed_storage().await;
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::with_commitment(1, commitment(100, false));
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        signer.clone(),
        provider.clone(),
        clock(201),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.submitted, 1);
    assert_eq!(signer.requests().await[0].batch_no, 1);
    assert_eq!(signer.requests().await[0].valid_until, 501);
    assert_eq!(provider.sent_count().await, 1);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(
        record.finalization_status,
        BatchFinalizationStatus::Submitted
    );
    assert_eq!(record.finalization_attempts, 1);
    assert_eq!(record.finalize_message_hash_norm, Some(hash(0x56)));
}

#[tokio::test]
async fn submitted_finalize_confirms_from_onchain_status_without_resend() {
    let storage = confirmed_storage().await;
    let mut record = storage.get_batch_commit(1).await.unwrap().unwrap();
    record.finalization_status = BatchFinalizationStatus::Submitted;
    record.finalize_message_hash_norm = Some(hash(0x55));
    storage.save_batch_commit(record).await.unwrap();
    let provider = MockProvider::with_commitment(1, commitment(100, true));
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        MockSigner::ok("EQsequencer"),
        provider,
        clock(300),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.finalized, 1);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(
        record.finalization_status,
        BatchFinalizationStatus::Finalized
    );
    assert_eq!(record.finalization_last_error, None);
}

#[tokio::test]
async fn bad_signer_sender_is_rejected_before_finalize_broadcast() {
    let storage = confirmed_storage().await;
    let signer = MockSigner::ok("EQattacker");
    let provider = MockProvider::with_commitment(1, commitment(100, false));
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        signer,
        provider.clone(),
        clock(201),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.failed, 1);
    assert_eq!(provider.sent_count().await, 0);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.finalization_status, BatchFinalizationStatus::Failed);
    assert_eq!(record.finalization_attempts, 1);
    assert_eq!(
        record.finalization_last_error.as_deref(),
        Some("finalize signer address mismatch")
    );
}

#[tokio::test]
async fn missing_onchain_commitment_fails_without_signing() {
    let storage = confirmed_storage().await;
    let signer = MockSigner::ok("EQsequencer");
    let finalizer = BatchFinalizer::new(
        config(),
        storage.clone(),
        signer.clone(),
        MockProvider::default(),
        clock(201),
    );

    let stats = finalizer.finalize_once().await.expect("finalize");

    assert_eq!(stats.failed, 1);
    assert!(signer.requests().await.is_empty());
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(
        record.finalization_last_error.as_deref(),
        Some("commitment missing")
    );
}

#[test]
fn parses_toncenter_commitment_getter_response() {
    let response =
        serde_json::json!({ "stack": [["num", "-1"], ["cell", commitment_boc(42, true)]] });

    let parsed = provider::parse_commitment_response(&response).expect("commitment");

    assert!(parsed.exists);
    assert_eq!(parsed.committed_at, Some(42));
    assert!(parsed.finalized);
}

#[derive(Clone, Default)]
struct MockSigner {
    signer_address: String,
    requests: Arc<Mutex<Vec<FinalizeBatchSignRequest>>>,
}

impl MockSigner {
    fn ok(signer_address: &str) -> Self {
        Self {
            signer_address: signer_address.to_owned(),
            requests: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn requests(&self) -> Vec<FinalizeBatchSignRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl FinalizeBatchSigner for MockSigner {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<SignedFinalizeBatch, FinalizerError> {
        self.requests.lock().await.push(request);
        Ok(SignedFinalizeBatch {
            boc_base64: "te6ccgEBAQEA".to_owned(),
            signer_address: self.signer_address.clone(),
        })
    }
}

#[derive(Clone, Default)]
struct MockProvider {
    commitments: Arc<Mutex<BTreeMap<u64, OnchainBatchCommitment>>>,
    sent: Arc<Mutex<Vec<SignedFinalizeBatch>>>,
}

impl MockProvider {
    fn with_commitment(batch_no: u64, commitment: OnchainBatchCommitment) -> Self {
        let mut commitments = BTreeMap::new();
        commitments.insert(batch_no, commitment);
        Self {
            commitments: Arc::new(Mutex::new(commitments)),
            sent: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn sent_count(&self) -> usize {
        self.sent.lock().await.len()
    }
}

#[async_trait]
impl TonFinalizerProvider for MockProvider {
    async fn send_signed_boc(
        &self,
        signed: &SignedFinalizeBatch,
    ) -> Result<TonSubmitResult, FinalizerError> {
        self.sent.lock().await.push(signed.clone());
        Ok(TonSubmitResult {
            message_hash: hash(0x55),
            message_hash_norm: hash(0x56),
        })
    }

    async fn message_confirmed(&self, _message_hash: Hash32) -> Result<bool, FinalizerError> {
        Ok(true)
    }

    async fn commitment(&self, batch_no: u64) -> Result<OnchainBatchCommitment, FinalizerError> {
        Ok(self
            .commitments
            .lock()
            .await
            .get(&batch_no)
            .cloned()
            .unwrap_or(OnchainBatchCommitment {
                exists: false,
                committed_at: None,
                finalized: false,
            }))
    }
}

async fn confirmed_storage() -> DynStorage {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    storage.save_block(block()).await.unwrap();
    let mut record = storage.get_batch_commit(1).await.unwrap().unwrap();
    record.status = BatchCommitStatus::Confirmed;
    storage.save_batch_commit(record).await.unwrap();
    storage
}

fn config() -> BatchFinalizerConfig {
    BatchFinalizerConfig {
        chain_id: "entropis-testnet".to_owned(),
        rollup_root_address: "EQroot".to_owned(),
        sender_address: "EQsequencer".to_owned(),
        finalize_msg_value_nanoton: 100_000_000,
        challenge_window_sec: 100,
        poll_interval_ms: 5_000,
        retry_backoff_ms: 15_000,
        max_attempts: 8,
    }
}

fn commitment(committed_at: u64, finalized: bool) -> OnchainBatchCommitment {
    OnchainBatchCommitment {
        exists: true,
        committed_at: Some(committed_at),
        finalized,
    }
}

fn clock(now: u64) -> MockClock {
    MockClock(now)
}

#[derive(Clone, Copy)]
struct MockClock(u64);

impl FinalizerClock for MockClock {
    fn unix_time(&self) -> u64 {
        self.0
    }
}

fn block() -> L2Block {
    L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        canonical_batch_data_hash(&[], &[]),
        100,
    )
}

fn commitment_boc(committed_at: u32, finalized: bool) -> String {
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;
    use tonlib_core::cell::{BagOfCells, CellBuilder};

    let roots_a = CellBuilder::new().build().expect("roots a");
    let roots_b = CellBuilder::new().build().expect("roots b");
    let cell = CellBuilder::new()
        .store_reference(&roots_a.to_arc())
        .expect("roots a ref")
        .store_reference(&roots_b.to_arc())
        .expect("roots b ref")
        .store_u32(32, committed_at)
        .expect("committed at")
        .store_bit(finalized)
        .expect("finalized")
        .build()
        .expect("commitment");
    let boc = BagOfCells::from_root(cell).serialize(false).expect("boc");
    BASE64_STANDARD.encode(boc)
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
