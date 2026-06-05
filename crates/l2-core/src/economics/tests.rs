use super::*;

#[test]
fn fee_split_must_sum_to_basis_points_and_allocate_without_losing_units() {
    let split = FeeSplit {
        burn_bps: 1_000,
        sequencer_bps: 7_500,
        treasury_bps: 1_500,
    };

    split.validate().expect("valid split");
    let allocation = split.allocate(123_456_789).expect("allocation");

    assert_eq!(allocation.total().unwrap(), 123_456_789);
    assert_eq!(allocation.burn, 12_345_678);
    assert_eq!(allocation.sequencer, 92_592_591);
    assert_eq!(allocation.treasury, 18_518_520);

    let invalid = FeeSplit {
        burn_bps: 1,
        sequencer_bps: 1,
        treasury_bps: 1,
    };
    assert!(matches!(
        invalid.validate(),
        Err(EconomicPolicyError::InvalidFeeSplit { sum_bps: 3 })
    ));
}

#[test]
fn fee_allocation_handles_max_amount_without_multiplication_overflow() {
    let split = FeeSplit {
        burn_bps: 3_333,
        sequencer_bps: 3_333,
        treasury_bps: 3_334,
    };

    let allocation = split.allocate(u128::MAX).expect("max allocation");

    assert_eq!(allocation.total().unwrap(), u128::MAX);
}

#[test]
fn bond_policy_requires_unbond_delay_beyond_challenge_window() {
    let valid = SecurityBondPolicy {
        min_sequencer_bond: 1,
        challenger_bond: 1,
        challenge_window_blocks: 100,
        unbond_delay_blocks: 101,
    };
    valid.validate().expect("valid bond policy");

    let early_unbond = SecurityBondPolicy {
        unbond_delay_blocks: 100,
        ..valid
    };
    assert!(matches!(
        early_unbond.validate(),
        Err(EconomicPolicyError::UnbondDelayNotLongerThanChallengeWindow)
    ));
}

#[test]
fn governance_and_reputation_policy_boundaries_are_explicit() {
    let governance = GovernancePolicy {
        proposal_timelock_blocks: 10,
        proposal_expiry_blocks: 20,
    };
    governance.validate().expect("valid governance policy");
    assert!(matches!(
        GovernancePolicy {
            proposal_timelock_blocks: 10,
            proposal_expiry_blocks: 10,
        }
        .validate(),
        Err(EconomicPolicyError::ProposalExpiryNotAfterTimelock)
    ));
    assert_eq!(GovernanceAction::EmergencyPause.validate(), Ok(()));
    assert!(matches!(
        GovernanceAction::ReleaseFunds.validate(),
        Err(EconomicPolicyError::GovernanceCannotReleaseFunds)
    ));

    let reputation = ReputationPolicy {
        min_score: -100,
        max_score: 100,
        recovery_step: 1,
    };
    reputation.validate().expect("valid reputation policy");
    assert!(matches!(
        ReputationPolicy {
            recovery_step: 0,
            ..reputation
        }
        .validate(),
        Err(EconomicPolicyError::ZeroReputationRecoveryStep)
    ));
}

#[test]
fn slashing_requires_nonzero_offender_batch_and_evidence_hash() {
    let valid = SlashingEvidence {
        kind: SlashingEvidenceKind::InvalidStateRoot,
        offender: hash(1),
        disputed_batch_no: 7,
        evidence_hash: hash(2),
    };
    valid.validate().expect("valid slashing evidence");

    assert!(matches!(
        SlashingEvidence {
            evidence_hash: Hash32::ZERO,
            ..valid
        }
        .validate(),
        Err(EconomicPolicyError::MissingSlashingEvidence)
    ));
    assert!(matches!(
        SlashingEvidence {
            offender: Hash32::ZERO,
            ..valid
        }
        .validate(),
        Err(EconomicPolicyError::ZeroSlashingOffender)
    ));
    assert!(matches!(
        SlashingEvidence {
            disputed_batch_no: 0,
            ..valid
        }
        .validate(),
        Err(EconomicPolicyError::ZeroDisputedBatch)
    ));
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
