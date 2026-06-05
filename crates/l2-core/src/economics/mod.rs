use crate::Hash32;
use serde::{Deserialize, Serialize};

pub const BASIS_POINTS_DENOMINATOR: u16 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeeSplit {
    pub burn_bps: u16,
    pub sequencer_bps: u16,
    pub treasury_bps: u16,
}

impl FeeSplit {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        let sum = u32::from(self.burn_bps)
            .checked_add(u32::from(self.sequencer_bps))
            .and_then(|sum| sum.checked_add(u32::from(self.treasury_bps)))
            .ok_or(EconomicPolicyError::FeeSplitOverflow)?;
        if sum != u32::from(BASIS_POINTS_DENOMINATOR) {
            return Err(EconomicPolicyError::InvalidFeeSplit { sum_bps: sum });
        }
        Ok(())
    }

    pub fn allocate(&self, amount: u128) -> Result<FeeAllocation, EconomicPolicyError> {
        self.validate()?;
        let burn = mul_div_bps(amount, self.burn_bps)?;
        let sequencer = mul_div_bps(amount, self.sequencer_bps)?;
        let assigned = burn
            .checked_add(sequencer)
            .ok_or(EconomicPolicyError::FeeAllocationOverflow)?;
        let treasury = amount
            .checked_sub(assigned)
            .ok_or(EconomicPolicyError::FeeAllocationOverflow)?;
        Ok(FeeAllocation {
            burn,
            sequencer,
            treasury,
        })
    }
}

impl Default for FeeSplit {
    fn default() -> Self {
        Self {
            burn_bps: 0,
            sequencer_bps: BASIS_POINTS_DENOMINATOR,
            treasury_bps: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeeAllocation {
    pub burn: u128,
    pub sequencer: u128,
    pub treasury: u128,
}

impl FeeAllocation {
    pub fn total(self) -> Result<u128, EconomicPolicyError> {
        self.burn
            .checked_add(self.sequencer)
            .and_then(|sum| sum.checked_add(self.treasury))
            .ok_or(EconomicPolicyError::FeeAllocationOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecurityBondPolicy {
    pub min_sequencer_bond: u128,
    pub challenger_bond: u128,
    pub challenge_window_blocks: u64,
    pub unbond_delay_blocks: u64,
}

impl SecurityBondPolicy {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        if self.min_sequencer_bond == 0 {
            return Err(EconomicPolicyError::ZeroSequencerBond);
        }
        if self.challenger_bond == 0 {
            return Err(EconomicPolicyError::ZeroChallengerBond);
        }
        if self.challenge_window_blocks == 0 {
            return Err(EconomicPolicyError::ZeroChallengeWindow);
        }
        if self.unbond_delay_blocks <= self.challenge_window_blocks {
            return Err(EconomicPolicyError::UnbondDelayNotLongerThanChallengeWindow);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GovernancePolicy {
    pub proposal_timelock_blocks: u64,
    pub proposal_expiry_blocks: u64,
}

impl GovernancePolicy {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        if self.proposal_timelock_blocks == 0 {
            return Err(EconomicPolicyError::ZeroGovernanceTimelock);
        }
        if self.proposal_expiry_blocks <= self.proposal_timelock_blocks {
            return Err(EconomicPolicyError::ProposalExpiryNotAfterTimelock);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReputationPolicy {
    pub min_score: i32,
    pub max_score: i32,
    pub recovery_step: u16,
}

impl ReputationPolicy {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        if self.min_score >= self.max_score {
            return Err(EconomicPolicyError::InvalidReputationRange);
        }
        if self.recovery_step == 0 {
            return Err(EconomicPolicyError::ZeroReputationRecoveryStep);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EconomicSecurityPolicy {
    pub fee_split: FeeSplit,
    pub bond: SecurityBondPolicy,
    pub governance: GovernancePolicy,
    pub reputation: ReputationPolicy,
}

impl EconomicSecurityPolicy {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        self.fee_split.validate()?;
        self.bond.validate()?;
        self.governance.validate()?;
        self.reputation.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SlashingEvidenceKind {
    InvalidStateRoot,
    DoubleSigning,
    FraudulentBridgeUpdate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SlashingEvidence {
    pub kind: SlashingEvidenceKind,
    pub offender: Hash32,
    pub disputed_batch_no: u64,
    pub evidence_hash: Hash32,
}

impl SlashingEvidence {
    pub fn validate(&self) -> Result<(), EconomicPolicyError> {
        if self.offender == Hash32::ZERO {
            return Err(EconomicPolicyError::ZeroSlashingOffender);
        }
        if self.disputed_batch_no == 0 {
            return Err(EconomicPolicyError::ZeroDisputedBatch);
        }
        if self.evidence_hash == Hash32::ZERO {
            return Err(EconomicPolicyError::MissingSlashingEvidence);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GovernanceAction {
    UpdateParameter,
    EmergencyPause,
    ReleaseFunds,
}

impl GovernanceAction {
    pub fn validate(self) -> Result<(), EconomicPolicyError> {
        match self {
            Self::UpdateParameter | Self::EmergencyPause => Ok(()),
            Self::ReleaseFunds => Err(EconomicPolicyError::GovernanceCannotReleaseFunds),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EconomicPolicyError {
    #[error("fee split overflows basis point sum")]
    FeeSplitOverflow,
    #[error("fee split basis points must sum to 10000, got {sum_bps}")]
    InvalidFeeSplit { sum_bps: u32 },
    #[error("fee allocation overflow")]
    FeeAllocationOverflow,
    #[error("sequencer bond must be non-zero")]
    ZeroSequencerBond,
    #[error("challenger bond must be non-zero")]
    ZeroChallengerBond,
    #[error("challenge window must be non-zero")]
    ZeroChallengeWindow,
    #[error("unbond delay must be longer than challenge window")]
    UnbondDelayNotLongerThanChallengeWindow,
    #[error("governance timelock must be non-zero")]
    ZeroGovernanceTimelock,
    #[error("proposal expiry must be after timelock")]
    ProposalExpiryNotAfterTimelock,
    #[error("reputation min score must be below max score")]
    InvalidReputationRange,
    #[error("reputation recovery step must be non-zero")]
    ZeroReputationRecoveryStep,
    #[error("slashing offender must be non-zero")]
    ZeroSlashingOffender,
    #[error("disputed batch must be non-zero")]
    ZeroDisputedBatch,
    #[error("slashing evidence hash must be non-zero")]
    MissingSlashingEvidence,
    #[error("governance cannot release bridged/user funds without proof")]
    GovernanceCannotReleaseFunds,
}

fn mul_div_bps(amount: u128, bps: u16) -> Result<u128, EconomicPolicyError> {
    let denominator = u128::from(BASIS_POINTS_DENOMINATOR);
    let bps = u128::from(bps);
    let whole = amount / denominator;
    let remainder = amount % denominator;
    let whole_component = whole
        .checked_mul(bps)
        .ok_or(EconomicPolicyError::FeeAllocationOverflow)?;
    let remainder_component = remainder
        .checked_mul(bps)
        .and_then(|value| value.checked_div(denominator))
        .ok_or(EconomicPolicyError::FeeAllocationOverflow)?;
    whole_component
        .checked_add(remainder_component)
        .ok_or(EconomicPolicyError::FeeAllocationOverflow)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
