use super::*;
use crate::da::{DaWriter, DataAvailabilityConfig, PublicDaBackend, StorageDaStore};
use crate::storage::{DynStorage, InMemoryStorage, StoredBatchPayload};
use l2_core::crypto::sha256_bytes;
use l2_core::{DepositEvent, Sequencer, SequencerConfig};
use std::sync::Arc;

#[tokio::test]
async fn observer_replays_valid_batch_and_stores_checkpoint() {
    let fixture = Fixture::new();
    let block = produce_deposit_blocks(&[b"deposit-a".as_slice()]).remove(0);
    fixture.write_da(&block).await;
    let commitment = crate::signer::BatchCommitment::from_block(&block).unwrap();

    let report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: true,
        })
        .await
        .expect("replay");
    let stored = fixture
        .storage
        .latest_observer_checkpoint()
        .await
        .expect("storage")
        .expect("checkpoint");

    assert_eq!(report.status, ObserverReplayStatus::Valid);
    assert_eq!(report.checked_batches, 1);
    assert_eq!(report.latest_checkpoint.state_root, block.header.state_root);
    assert_eq!(stored, report.latest_checkpoint);
}

#[tokio::test]
async fn observer_detects_tampered_state_root() {
    let fixture = Fixture::new();
    let block = produce_deposit_blocks(&[b"deposit-a".as_slice()]).remove(0);
    fixture.write_da(&block).await;
    let mut commitment = crate::signer::BatchCommitment::from_block(&block).unwrap();
    commitment.roots_a.state_root = sha256_bytes(b"malicious-state-root");

    let report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: false,
        })
        .await
        .expect("replay");
    let divergence = report.first_divergence.expect("divergence");

    assert_eq!(report.status, ObserverReplayStatus::Invalid);
    assert_eq!(report.checked_batches, 0);
    assert_eq!(divergence.kind, DivergenceKind::RootMismatch);
    assert_eq!(divergence.field, Some("state_root"));

    let witness = report.challenge_witness.expect("challenge witness");
    assert_eq!(
        witness.challenge_kind,
        crate::observer::ChallengeKind::InvalidTransition
    );
    assert_eq!(witness.l1_inputs.message, "ChallengeBatch");
    assert_eq!(witness.l1_inputs.challenge_kind_code, 2);
    assert_eq!(witness.l1_inputs.batch_no, 1);
    assert_eq!(witness.l1_inputs.field, Some("state_root"));
    assert_eq!(
        witness.l1_inputs.expected_root,
        Some(block.header.state_root)
    );
    assert_eq!(
        witness.l1_inputs.claimed_root,
        Some(sha256_bytes(b"malicious-state-root"))
    );
    assert!(witness.validate_integrity());
}

#[tokio::test]
async fn observer_reports_missing_da_separately() {
    let fixture = Fixture::new();
    let block = produce_deposit_blocks(&[b"deposit-a".as_slice()]).remove(0);
    let commitment = crate::signer::BatchCommitment::from_block(&block).unwrap();

    let report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: false,
        })
        .await
        .expect("replay");
    let divergence = report.first_divergence.expect("divergence");

    assert_eq!(report.status, ObserverReplayStatus::MissingDa);
    assert_eq!(divergence.kind, DivergenceKind::MissingDa);
    assert_eq!(divergence.reason, "batch data unavailable");

    let witness = report.challenge_witness.expect("challenge witness");
    assert_eq!(
        witness.challenge_kind,
        crate::observer::ChallengeKind::MissingDa
    );
    assert_eq!(witness.l1_inputs.challenge_kind_code, 1);
    assert_eq!(witness.l1_inputs.expected_root, None);
    assert_eq!(witness.l1_inputs.claimed_root, Some(block.header.data_hash));
    assert_eq!(
        witness.path.timeout_rule,
        "sequencer must provide DA before response deadline"
    );
    assert!(witness.validate_integrity());
}

#[tokio::test]
async fn observer_rejects_corrupted_da_payload() {
    let fixture = Fixture::new();
    let block = produce_deposit_blocks(&[b"deposit-a".as_slice()]).remove(0);
    let commitment = crate::signer::BatchCommitment::from_block(&block).unwrap();
    fixture
        .storage
        .save_batch_payload(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: vec![0, 1, 2],
            public_ref: None,
            public_uri: None,
        })
        .await
        .expect("corrupt da");

    let report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: false,
        })
        .await
        .expect("replay");
    let divergence = report.first_divergence.expect("divergence");

    assert_eq!(report.status, ObserverReplayStatus::CorruptDa);
    assert_eq!(divergence.kind, DivergenceKind::CorruptDa);
    assert_eq!(divergence.field, Some("data_hash"));
}

#[tokio::test]
async fn challenge_witness_integrity_detects_manipulation() {
    let fixture = Fixture::new();
    let block = produce_deposit_blocks(&[b"deposit-a".as_slice()]).remove(0);
    fixture.write_da(&block).await;
    let mut commitment = crate::signer::BatchCommitment::from_block(&block).unwrap();
    commitment.roots_a.state_root = sha256_bytes(b"malicious-state-root");

    let report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: false,
        })
        .await
        .expect("replay");
    let mut witness = report.challenge_witness.expect("challenge witness");
    assert!(witness.validate_integrity());

    witness.l1_inputs.claimed_root = Some(sha256_bytes(b"rewritten-claim"));
    assert!(!witness.validate_integrity());
}

#[tokio::test]
async fn observer_replay_is_deterministic_over_multiple_blocks() {
    let fixture = Fixture::new();
    let mut blocks = produce_deposit_blocks(&[b"deposit-a".as_slice(), b"deposit-b".as_slice()]);
    let first = blocks.remove(0);
    let second = blocks.remove(0);
    fixture.write_da(&first).await;
    fixture.write_da(&second).await;
    let commitments = vec![
        crate::signer::BatchCommitment::from_block(&first).unwrap(),
        crate::signer::BatchCommitment::from_block(&second).unwrap(),
    ];

    let first_report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: Some(ObserverCheckpoint::genesis()),
            commitments: commitments.clone(),
            store_checkpoint: false,
        })
        .await
        .expect("first replay");
    let second_report = fixture
        .service
        .replay(ObserverReplayRequest {
            trusted_checkpoint: Some(ObserverCheckpoint::genesis()),
            commitments,
            store_checkpoint: false,
        })
        .await
        .expect("second replay");

    assert_eq!(first_report.status, ObserverReplayStatus::Valid);
    assert_eq!(first_report.checked_batches, 2);
    assert_eq!(
        first_report.latest_checkpoint,
        second_report.latest_checkpoint
    );
    assert_eq!(
        first_report.latest_checkpoint.state_root,
        second.header.state_root
    );
}

struct Fixture {
    storage: DynStorage,
    da: Arc<StorageDaStore>,
    service: ObserverReplayService,
}

impl Fixture {
    fn new() -> Self {
        let storage: DynStorage = Arc::new(InMemoryStorage::default());
        let da = Arc::new(StorageDaStore::new(
            storage.clone(),
            DataAvailabilityConfig {
                max_payload_bytes: 1024 * 1024,
                public_backend: PublicDaBackend::PostgresOnly,
            },
        ));
        let service = ObserverReplayService::new(
            storage.clone(),
            da.clone(),
            ObserverReplayConfig::default(),
        );
        Self {
            storage,
            da,
            service,
        }
    }

    async fn write_da(&self, block: &l2_core::L2Block) {
        self.da.write_batch_payload(block).await.expect("write da");
    }
}

fn produce_deposit_blocks(labels: &[&[u8]]) -> Vec<l2_core::L2Block> {
    let mut sequencer = Sequencer::new(SequencerConfig {
        chain_id: "entropis-testnet".to_owned(),
        ..SequencerConfig::default()
    });
    labels
        .iter()
        .map(|label| {
            sequencer.ingest_deposits(vec![deposit_event(label)]);
            sequencer.produce_block(label.len() as u64).expect("block")
        })
        .collect()
}

fn deposit_event(label: &[u8]) -> DepositEvent {
    DepositEvent {
        deposit_id: sha256_bytes(label),
        asset_id: 0,
        recipient: sha256_bytes(b"recipient"),
        amount: 100,
        l1_tx_hash: sha256_bytes(&[label, b"l1"].concat()),
        l1_lt: label.len() as u64 + 1,
    }
}
