use super::*;
use crate::storage::{BatchCommitStatus, DynStorage, InMemoryStorage};
use l2_core::crypto::sha256_bytes;
use std::sync::Arc;
use tokio::sync::Mutex;

#[test]
fn commitment_maps_block_header_to_rollup_roots() {
    let block = block(0, sha256_bytes(b"prev-state"), sha256_bytes(b"state"));

    let commitment = BatchCommitment::from_block(&block).expect("commitment");

    assert_eq!(commitment.batch_no, 1);
    assert_eq!(commitment.block_height, 0);
    assert_eq!(commitment.block_hash, block.header.block_hash());
    assert_eq!(
        commitment.roots_a.prev_state_root,
        block.header.prev_state_root
    );
    assert_eq!(commitment.roots_a.state_root, block.header.state_root);
    assert_eq!(commitment.roots_a.tx_root, block.header.tx_root);
    assert_eq!(commitment.roots_b.receipt_root, block.header.receipt_root);
    assert_eq!(
        commitment.roots_b.withdrawal_root,
        block.header.withdrawal_root
    );
    assert_eq!(commitment.roots_b.data_hash, block.header.data_hash);
}

#[tokio::test]
async fn relay_submits_pending_block_and_stores_submitted_status() {
    let storage = storage_with_block(block(0, Hash32::ZERO, sha256_bytes(b"state"))).await;
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::ok(hash(0x44));
    let relayer = BatchRelayer::new(config(), storage.clone(), signer.clone(), provider.clone());

    let stats = relayer.relay_once().await.expect("relay");

    assert_eq!(stats.submitted, 1);
    assert_eq!(signer.requests().await.len(), 1);
    assert_eq!(provider.sent_count().await, 1);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchCommitStatus::Submitted);
    assert_eq!(record.attempts, 1);
    assert_eq!(record.message_hash_norm, Some(hash(0x45)));
}

#[tokio::test]
async fn submitted_batch_is_not_sent_twice_and_can_confirm() {
    let storage = storage_with_block(block(0, Hash32::ZERO, sha256_bytes(b"state"))).await;
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::ok(hash(0x44));
    let relayer = BatchRelayer::new(config(), storage.clone(), signer, provider.clone());

    relayer.relay_once().await.expect("submit");
    let second = relayer.relay_once().await.expect("confirm");

    assert_eq!(second.submitted, 0);
    assert_eq!(second.confirmed, 1);
    assert_eq!(provider.sent_count().await, 1);
    assert_eq!(
        storage.get_batch_commit(1).await.unwrap().unwrap().status,
        BatchCommitStatus::Confirmed
    );
}

#[tokio::test]
async fn failed_send_retries_until_max_attempts() {
    let storage = storage_with_block(block(0, Hash32::ZERO, sha256_bytes(b"state"))).await;
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::failing_send();
    let mut config = config();
    config.max_attempts = 2;
    let relayer = BatchRelayer::new(config, storage.clone(), signer, provider.clone());

    assert_eq!(relayer.relay_once().await.unwrap().failed, 1);
    assert_eq!(relayer.relay_once().await.unwrap().failed, 1);
    assert_eq!(relayer.relay_once().await.unwrap().considered, 0);

    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchCommitStatus::Failed);
    assert_eq!(record.attempts, 2);
    assert_eq!(provider.sent_count().await, 2);
}

#[tokio::test]
async fn bad_signer_sender_is_rejected_before_provider_send() {
    let storage = storage_with_block(block(0, Hash32::ZERO, sha256_bytes(b"state"))).await;
    let relayer = BatchRelayer::new(
        config(),
        storage.clone(),
        MockSigner::ok("EQattacker"),
        MockProvider::ok(hash(0x44)),
    );

    let stats = relayer.relay_once().await.expect("relay");

    assert_eq!(stats.failed, 1);
    let record = storage.get_batch_commit(1).await.unwrap().unwrap();
    assert_eq!(record.status, BatchCommitStatus::Failed);
    assert_eq!(record.attempts, 1);
    assert_eq!(
        record.last_error.as_deref(),
        Some("commit signer address mismatch")
    );
}

#[tokio::test]
async fn block_hash_mismatch_fails_without_sending() {
    let storage = storage_with_block(block(0, Hash32::ZERO, sha256_bytes(b"state"))).await;
    let mut record = storage.get_batch_commit(1).await.unwrap().unwrap();
    record.block_hash = hash(0x99);
    storage.save_batch_commit(record).await.unwrap();
    let signer = MockSigner::ok("EQsequencer");
    let provider = MockProvider::ok(hash(0x44));
    let relayer = BatchRelayer::new(config(), storage.clone(), signer.clone(), provider.clone());

    let stats = relayer.relay_once().await.expect("relay");

    assert_eq!(stats.failed, 1);
    assert!(signer.requests().await.is_empty());
    assert_eq!(provider.sent_count().await, 0);
    assert_eq!(
        storage
            .get_batch_commit(1)
            .await
            .unwrap()
            .unwrap()
            .last_error
            .as_deref(),
        Some("l2 block hash mismatch")
    );
}

#[derive(Clone, Default)]
struct MockSigner {
    signer_address: String,
    requests: Arc<Mutex<Vec<CommitBatchSignRequest>>>,
}

impl MockSigner {
    fn ok(signer_address: &str) -> Self {
        Self {
            signer_address: signer_address.to_owned(),
            requests: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn requests(&self) -> Vec<CommitBatchSignRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl CommitBatchSigner for MockSigner {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, RelayerError> {
        self.requests.lock().await.push(request);
        Ok(SignedCommitBatch {
            boc_base64: "te6ccgEBAQEA".to_owned(),
            signer_address: self.signer_address.clone(),
        })
    }
}

#[derive(Clone, Default)]
struct MockProvider {
    send_error: bool,
    message_hash: Hash32,
    sent: Arc<Mutex<Vec<SignedCommitBatch>>>,
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

#[async_trait]
impl TonCommitProvider for MockProvider {
    async fn send_signed_boc(
        &self,
        signed: &SignedCommitBatch,
    ) -> Result<TonSubmitResult, RelayerError> {
        if self.send_error {
            self.sent.lock().await.push(signed.clone());
            return Err(RelayerError::Provider("network".to_owned()));
        }
        self.sent.lock().await.push(signed.clone());
        Ok(TonSubmitResult {
            message_hash: self.message_hash,
            message_hash_norm: hash(self.message_hash.as_bytes()[0] + 1),
        })
    }

    async fn message_confirmed(&self, _message_hash: Hash32) -> Result<bool, RelayerError> {
        Ok(true)
    }
}

async fn storage_with_block(block: L2Block) -> DynStorage {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    storage.save_block(block).await.unwrap();
    storage
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

fn block(height: u64, prev_state_root: Hash32, state_root: Hash32) -> L2Block {
    L2Block::new(
        height,
        Hash32::ZERO,
        prev_state_root,
        state_root,
        vec![],
        vec![],
        vec![],
        sha256_bytes(b"data"),
        100,
    )
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
