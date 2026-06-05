use crate::address::is_l2_zero_address;
use crate::crypto::Hash32;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposerSelectionMode {
    SingleTrusted,
    StakeWeightedPreview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposerStatus {
    Active,
    Standby,
    Suspended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposerIdentity {
    pub proposer_id: Hash32,
    pub signer_account: Hash32,
    pub reward_account: Hash32,
    pub status: ProposerStatus,
    pub stake_weight: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposalObservation {
    pub pending_user_txs: u64,
    pub included_user_txs: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposerSetSignals {
    pub proposals: u64,
    pub missed_slots: u64,
    pub unauthorized_proposals: u64,
    pub duplicate_proposals: u64,
    pub out_of_order_proposals: u64,
    pub censorship_signals: u64,
    pub pending_user_txs: u64,
    pub included_user_txs: u64,
    pub last_height: Option<u64>,
    pub last_proposer: Option<Hash32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProposerSet {
    pub mode: ProposerSelectionMode,
    pub trusted_proposer: Hash32,
    pub proposers: BTreeMap<Hash32, ProposerIdentity>,
    pub proposed_heights: BTreeSet<u64>,
    pub signals: ProposerSetSignals,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProposerSetError {
    #[error("proposer identity is invalid")]
    InvalidIdentity,
    #[error("proposer set has no active proposers")]
    NoActiveProposer,
    #[error("proposer is unknown")]
    UnknownProposer,
    #[error("proposer is not active")]
    InactiveProposer,
    #[error("proposer is not expected for this height")]
    UnexpectedProposer,
    #[error("proposal height was already recorded")]
    DuplicateProposal,
    #[error("proposal height is out of order")]
    OutOfOrderProposal,
}

impl ProposerSetError {
    pub fn rejection_reason(self) -> &'static str {
        match self {
            Self::InvalidIdentity => "proposer_invalid_identity",
            Self::NoActiveProposer => "proposer_no_active",
            Self::UnknownProposer => "proposer_unknown",
            Self::InactiveProposer => "proposer_inactive",
            Self::UnexpectedProposer => "proposer_unexpected",
            Self::DuplicateProposal => "proposer_duplicate_height",
            Self::OutOfOrderProposal => "proposer_out_of_order",
        }
    }
}

impl ProposerIdentity {
    pub fn active(proposer_id: Hash32, signer_account: Hash32, reward_account: Hash32) -> Self {
        Self {
            proposer_id,
            signer_account,
            reward_account,
            status: ProposerStatus::Active,
            stake_weight: 1,
        }
    }

    fn validate(self) -> Result<(), ProposerSetError> {
        if is_l2_zero_address(self.proposer_id)
            || is_l2_zero_address(self.signer_account)
            || is_l2_zero_address(self.reward_account)
        {
            return Err(ProposerSetError::InvalidIdentity);
        }
        if self.status == ProposerStatus::Active && self.stake_weight == 0 {
            return Err(ProposerSetError::InvalidIdentity);
        }
        Ok(())
    }
}

impl ProposerSet {
    pub fn single_trusted(proposer_id: Hash32) -> Result<Self, ProposerSetError> {
        let identity = ProposerIdentity::active(proposer_id, proposer_id, proposer_id);
        Self::new(
            ProposerSelectionMode::SingleTrusted,
            proposer_id,
            vec![identity],
        )
    }

    pub fn new(
        mode: ProposerSelectionMode,
        trusted_proposer: Hash32,
        identities: Vec<ProposerIdentity>,
    ) -> Result<Self, ProposerSetError> {
        if is_l2_zero_address(trusted_proposer) {
            return Err(ProposerSetError::InvalidIdentity);
        }
        let mut proposers = BTreeMap::new();
        for identity in identities {
            identity.validate()?;
            if proposers.insert(identity.proposer_id, identity).is_some() {
                return Err(ProposerSetError::InvalidIdentity);
            }
        }
        if !proposers
            .get(&trusted_proposer)
            .is_some_and(|proposer| proposer.status == ProposerStatus::Active)
        {
            return Err(ProposerSetError::NoActiveProposer);
        }
        let set = Self {
            mode,
            trusted_proposer,
            proposers,
            proposed_heights: BTreeSet::new(),
            signals: ProposerSetSignals::default(),
        };
        set.expected_proposer(0)?;
        Ok(set)
    }

    pub fn expected_proposer(&self, height: u64) -> Result<Hash32, ProposerSetError> {
        match self.mode {
            ProposerSelectionMode::SingleTrusted => Ok(self.trusted_proposer),
            ProposerSelectionMode::StakeWeightedPreview => {
                let active = self.active_weighted_order();
                if active.is_empty() {
                    return Err(ProposerSetError::NoActiveProposer);
                }
                Ok(active[(height as usize) % active.len()])
            }
        }
    }

    pub fn record_proposal(
        &mut self,
        proposer_id: Hash32,
        height: u64,
        observation: ProposalObservation,
    ) -> Result<(), ProposerSetError> {
        if let Err(error) = self.validate_proposal(proposer_id, height) {
            self.record_validation_error(error);
            return Err(error);
        }
        self.proposed_heights.insert(height);
        self.signals.proposals = self.signals.proposals.saturating_add(1);
        self.signals.last_height = Some(height);
        self.signals.last_proposer = Some(proposer_id);
        self.signals.pending_user_txs = self
            .signals
            .pending_user_txs
            .saturating_add(observation.pending_user_txs);
        self.signals.included_user_txs = self
            .signals
            .included_user_txs
            .saturating_add(observation.included_user_txs);
        if observation.pending_user_txs > observation.included_user_txs {
            self.signals.censorship_signals = self.signals.censorship_signals.saturating_add(
                observation
                    .pending_user_txs
                    .saturating_sub(observation.included_user_txs),
            );
        }
        Ok(())
    }

    pub fn record_missed_slot(&mut self, height: u64) -> Result<Hash32, ProposerSetError> {
        let expected = self.expected_proposer(height)?;
        self.signals.missed_slots = self.signals.missed_slots.saturating_add(1);
        Ok(expected)
    }

    fn validate_proposal(&self, proposer_id: Hash32, height: u64) -> Result<(), ProposerSetError> {
        let proposer = self
            .proposers
            .get(&proposer_id)
            .ok_or(ProposerSetError::UnknownProposer)?;
        if proposer.status != ProposerStatus::Active {
            return Err(ProposerSetError::InactiveProposer);
        }
        let expected = self.expected_proposer(height)?;
        if expected != proposer_id {
            return Err(ProposerSetError::UnexpectedProposer);
        }
        if self.proposed_heights.contains(&height) {
            return Err(ProposerSetError::DuplicateProposal);
        }
        if self
            .signals
            .last_height
            .is_some_and(|last_height| height != last_height.saturating_add(1))
        {
            return Err(ProposerSetError::OutOfOrderProposal);
        }
        if self.signals.last_height.is_none() && height != 0 {
            return Err(ProposerSetError::OutOfOrderProposal);
        }
        Ok(())
    }

    fn record_validation_error(&mut self, error: ProposerSetError) {
        match error {
            ProposerSetError::UnknownProposer
            | ProposerSetError::InactiveProposer
            | ProposerSetError::UnexpectedProposer => {
                self.signals.unauthorized_proposals =
                    self.signals.unauthorized_proposals.saturating_add(1);
            }
            ProposerSetError::DuplicateProposal => {
                self.signals.duplicate_proposals =
                    self.signals.duplicate_proposals.saturating_add(1);
            }
            ProposerSetError::OutOfOrderProposal => {
                self.signals.out_of_order_proposals =
                    self.signals.out_of_order_proposals.saturating_add(1);
            }
            ProposerSetError::InvalidIdentity | ProposerSetError::NoActiveProposer => {}
        }
    }

    fn active_weighted_order(&self) -> Vec<Hash32> {
        let mut active = self
            .proposers
            .values()
            .filter(|proposer| proposer.status == ProposerStatus::Active)
            .collect::<Vec<_>>();
        active.sort_by(|left, right| {
            right
                .stake_weight
                .cmp(&left.stake_weight)
                .then_with(|| left.proposer_id.cmp(&right.proposer_id))
        });
        active
            .into_iter()
            .map(|proposer| proposer.proposer_id)
            .collect()
    }
}
