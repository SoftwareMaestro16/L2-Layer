use super::{DivergenceKind, ObserverDivergence, ObserverReplayStatus};
use crate::signer::BatchCommitment;
use crate::storage::ObserverCheckpoint;
use l2_core::crypto::hash_domain;
use l2_core::Hash32;
use serde::Serialize;

pub const CHALLENGE_WITNESS_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeKind {
    MissingDa,
    InvalidTransition,
}

impl ChallengeKind {
    pub fn l1_code(self) -> u8 {
        match self {
            Self::MissingDa => 1,
            Self::InvalidTransition => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChallengeWitness {
    pub version: u8,
    pub challenge_kind: ChallengeKind,
    pub l1_inputs: L1ChallengeInputs,
    pub checkpoint: ChallengeCheckpointSummary,
    pub commitment: ChallengeCommitmentSummary,
    pub divergence: ObserverDivergence,
    pub path: L1ChallengePath,
}

impl ChallengeWitness {
    pub fn from_divergence(
        checkpoint: &ObserverCheckpoint,
        commitment: &BatchCommitment,
        status: ObserverReplayStatus,
        divergence: &ObserverDivergence,
    ) -> Option<Self> {
        let challenge_kind = challenge_kind(status, divergence.kind)?;
        let mut l1_inputs = L1ChallengeInputs::from_divergence(challenge_kind, divergence);
        if challenge_kind == ChallengeKind::MissingDa && l1_inputs.claimed_root.is_none() {
            l1_inputs.claimed_root = Some(commitment.roots_b.data_hash);
        }
        let mut witness = Self {
            version: CHALLENGE_WITNESS_VERSION,
            challenge_kind,
            l1_inputs,
            checkpoint: ChallengeCheckpointSummary::from_checkpoint(checkpoint),
            commitment: ChallengeCommitmentSummary::from_commitment(commitment),
            divergence: divergence.clone(),
            path: L1ChallengePath::for_kind(challenge_kind),
        };
        witness.l1_inputs.evidence_hash = witness.recompute_evidence_hash();
        Some(witness)
    }

    pub fn validate_integrity(&self) -> bool {
        self.l1_inputs.evidence_hash == self.recompute_evidence_hash()
    }

    pub fn recompute_evidence_hash(&self) -> Hash32 {
        let mut encoded = Vec::new();
        encoded.push(self.version);
        encoded.push(self.challenge_kind.l1_code());
        push_u64(&mut encoded, self.l1_inputs.batch_no);
        push_u64(&mut encoded, self.l1_inputs.block_height);
        push_option_u32(&mut encoded, self.l1_inputs.disputed_tx_index);
        push_str(&mut encoded, self.l1_inputs.field.unwrap_or(""));
        push_option_hash(&mut encoded, self.l1_inputs.expected_root);
        push_option_hash(&mut encoded, self.l1_inputs.claimed_root);
        push_u64(&mut encoded, self.checkpoint.next_batch_no);
        push_u64(&mut encoded, self.checkpoint.next_block_height);
        push_hash(&mut encoded, self.checkpoint.state_root);
        push_hash(&mut encoded, self.checkpoint.integrity_hash);
        push_hash(&mut encoded, self.commitment.block_hash);
        push_hash(&mut encoded, self.commitment.prev_state_root);
        push_hash(&mut encoded, self.commitment.state_root);
        push_hash(&mut encoded, self.commitment.tx_root);
        push_hash(&mut encoded, self.commitment.receipt_root);
        push_hash(&mut encoded, self.commitment.withdrawal_root);
        push_hash(&mut encoded, self.commitment.data_hash);
        push_str(&mut encoded, self.divergence.reason);
        hash_domain("l2.challenge.witness.v1", &[&encoded])
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct L1ChallengeInputs {
    pub message: &'static str,
    pub batch_no: u64,
    pub block_height: u64,
    pub challenge_kind_code: u8,
    pub challenge_kind: ChallengeKind,
    pub disputed_tx_index: Option<u32>,
    pub field: Option<&'static str>,
    pub expected_root: Option<Hash32>,
    pub claimed_root: Option<Hash32>,
    pub evidence_hash: Hash32,
}

impl L1ChallengeInputs {
    fn from_divergence(challenge_kind: ChallengeKind, divergence: &ObserverDivergence) -> Self {
        let (expected_root, claimed_root) = match challenge_kind {
            ChallengeKind::MissingDa => (None, divergence.expected_hash.or(divergence.actual_hash)),
            ChallengeKind::InvalidTransition => (divergence.actual_hash, divergence.expected_hash),
        };
        Self {
            message: "ChallengeBatch",
            batch_no: divergence.batch_no,
            block_height: divergence.block_height,
            challenge_kind_code: challenge_kind.l1_code(),
            challenge_kind,
            disputed_tx_index: checked_tx_index(divergence.tx_index),
            field: divergence.field,
            expected_root,
            claimed_root,
            evidence_hash: Hash32::ZERO,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChallengeCheckpointSummary {
    pub next_batch_no: u64,
    pub next_block_height: u64,
    pub state_root: Hash32,
    pub integrity_hash: Hash32,
}

impl ChallengeCheckpointSummary {
    fn from_checkpoint(checkpoint: &ObserverCheckpoint) -> Self {
        let mut encoded = Vec::new();
        push_u64(&mut encoded, checkpoint.next_batch_no);
        push_u64(&mut encoded, checkpoint.next_block_height);
        push_hash(&mut encoded, checkpoint.state_root);
        Self {
            next_batch_no: checkpoint.next_batch_no,
            next_block_height: checkpoint.next_block_height,
            state_root: checkpoint.state_root,
            integrity_hash: hash_domain("l2.challenge.checkpoint.v1", &[&encoded]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChallengeCommitmentSummary {
    pub batch_no: u64,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub prev_state_root: Hash32,
    pub state_root: Hash32,
    pub tx_root: Hash32,
    pub receipt_root: Hash32,
    pub withdrawal_root: Hash32,
    pub data_hash: Hash32,
}

impl ChallengeCommitmentSummary {
    fn from_commitment(commitment: &BatchCommitment) -> Self {
        Self {
            batch_no: commitment.batch_no,
            block_height: commitment.block_height,
            block_hash: commitment.block_hash,
            prev_state_root: commitment.roots_a.prev_state_root,
            state_root: commitment.roots_a.state_root,
            tx_root: commitment.roots_a.tx_root,
            receipt_root: commitment.roots_b.receipt_root,
            withdrawal_root: commitment.roots_b.withdrawal_root,
            data_hash: commitment.roots_b.data_hash,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct L1ChallengePath {
    pub open_message: &'static str,
    pub response_message: &'static str,
    pub resolution_message: &'static str,
    pub timeout_rule: &'static str,
}

impl L1ChallengePath {
    fn for_kind(kind: ChallengeKind) -> Self {
        match kind {
            ChallengeKind::MissingDa => Self {
                open_message: "ChallengeBatch",
                response_message: "RespondChallenge",
                resolution_message: "ResolveChallenge",
                timeout_rule: "sequencer must provide DA before response deadline",
            },
            ChallengeKind::InvalidTransition => Self {
                open_message: "ChallengeBatch",
                response_message: "RespondChallenge",
                resolution_message: "ResolveChallenge",
                timeout_rule: "batch cannot finalize while invalid-transition challenge is open",
            },
        }
    }
}

fn challenge_kind(
    status: ObserverReplayStatus,
    divergence: DivergenceKind,
) -> Option<ChallengeKind> {
    match (status, divergence) {
        (ObserverReplayStatus::MissingDa, DivergenceKind::MissingDa)
        | (ObserverReplayStatus::CorruptDa, DivergenceKind::CorruptDa) => {
            Some(ChallengeKind::MissingDa)
        }
        (ObserverReplayStatus::Invalid, DivergenceKind::ReceiptMismatch)
        | (ObserverReplayStatus::Invalid, DivergenceKind::RootMismatch) => {
            Some(ChallengeKind::InvalidTransition)
        }
        _ => None,
    }
}

fn checked_tx_index(index: Option<usize>) -> Option<u32> {
    index.and_then(|value| u32::try_from(value).ok())
}

fn push_hash(out: &mut Vec<u8>, hash: Hash32) {
    out.extend_from_slice(hash.as_bytes());
}

fn push_option_hash(out: &mut Vec<u8>, hash: Option<Hash32>) {
    match hash {
        Some(hash) => {
            out.push(1);
            push_hash(out, hash);
        }
        None => out.push(0),
    }
}

fn push_option_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_be_bytes());
        }
        None => out.push(0),
    }
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}
