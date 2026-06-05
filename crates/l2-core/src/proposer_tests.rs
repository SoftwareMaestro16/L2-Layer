use crate::crypto::sha256_bytes;
use crate::{
    ProposalObservation, ProposerIdentity, ProposerSelectionMode, ProposerSet, ProposerSetError,
    ProposerStatus,
};

fn id(label: &[u8]) -> crate::Hash32 {
    sha256_bytes(label)
}

fn identity(label: &[u8], weight: u128) -> ProposerIdentity {
    let proposer_id = id(label);
    let mut signer_label = label.to_vec();
    signer_label.extend_from_slice(b"-signer");
    let mut reward_label = label.to_vec();
    reward_label.extend_from_slice(b"-reward");
    ProposerIdentity {
        proposer_id,
        signer_account: id(&signer_label),
        reward_account: id(&reward_label),
        status: ProposerStatus::Active,
        stake_weight: weight,
    }
}

#[test]
fn single_trusted_set_rejects_unauthorized_duplicate_and_out_of_order() {
    let trusted = id(b"trusted");
    let attacker = id(b"attacker");
    let mut set = ProposerSet::single_trusted(trusted).expect("single trusted");

    set.record_proposal(
        trusted,
        0,
        ProposalObservation {
            pending_user_txs: 3,
            included_user_txs: 3,
        },
    )
    .expect("height zero");

    assert_eq!(
        set.record_proposal(attacker, 1, ProposalObservation::default()),
        Err(ProposerSetError::UnknownProposer)
    );
    assert_eq!(set.signals.unauthorized_proposals, 1);

    assert_eq!(
        set.record_proposal(trusted, 0, ProposalObservation::default()),
        Err(ProposerSetError::DuplicateProposal)
    );
    assert_eq!(set.signals.duplicate_proposals, 1);

    assert_eq!(
        set.record_proposal(trusted, 3, ProposalObservation::default()),
        Err(ProposerSetError::OutOfOrderProposal)
    );
    assert_eq!(set.signals.out_of_order_proposals, 1);
    assert_eq!(set.signals.proposals, 1);
}

#[test]
fn stake_weighted_preview_is_deterministic_and_skips_suspended() {
    let first = identity(b"first", 10);
    let second = identity(b"second", 50);
    let mut third = identity(b"third", 100);
    third.status = ProposerStatus::Suspended;
    let mut set = ProposerSet::new(
        ProposerSelectionMode::StakeWeightedPreview,
        second.proposer_id,
        vec![first, second, third],
    )
    .expect("weighted set");

    assert_eq!(set.expected_proposer(0), Ok(second.proposer_id));
    assert_eq!(set.expected_proposer(1), Ok(first.proposer_id));
    assert_eq!(set.expected_proposer(2), Ok(second.proposer_id));

    assert_eq!(
        set.record_proposal(first.proposer_id, 0, ProposalObservation::default()),
        Err(ProposerSetError::UnexpectedProposer)
    );
    assert_eq!(set.signals.unauthorized_proposals, 1);
}

#[test]
fn censorship_and_missed_slot_signals_are_observable() {
    let trusted = id(b"trusted");
    let mut set = ProposerSet::single_trusted(trusted).expect("single trusted");

    set.record_proposal(
        trusted,
        0,
        ProposalObservation {
            pending_user_txs: 9,
            included_user_txs: 4,
        },
    )
    .expect("proposal");
    let missed = set.record_missed_slot(1).expect("missed slot");

    assert_eq!(missed, trusted);
    assert_eq!(set.signals.censorship_signals, 5);
    assert_eq!(set.signals.pending_user_txs, 9);
    assert_eq!(set.signals.included_user_txs, 4);
    assert_eq!(set.signals.missed_slots, 1);
    assert_eq!(set.signals.last_height, Some(0));
    assert_eq!(set.signals.last_proposer, Some(trusted));
}

#[test]
fn invalid_identity_config_fails_closed() {
    let zero = crate::Hash32::ZERO;
    assert_eq!(
        ProposerSet::single_trusted(zero),
        Err(ProposerSetError::InvalidIdentity)
    );

    let mut inactive = identity(b"inactive", 1);
    inactive.status = ProposerStatus::Standby;
    assert_eq!(
        ProposerSet::new(
            ProposerSelectionMode::SingleTrusted,
            inactive.proposer_id,
            vec![inactive],
        ),
        Err(ProposerSetError::NoActiveProposer)
    );

    let mut zero_weight = identity(b"zero-weight", 0);
    zero_weight.status = ProposerStatus::Active;
    assert_eq!(
        ProposerSet::new(
            ProposerSelectionMode::SingleTrusted,
            zero_weight.proposer_id,
            vec![zero_weight],
        ),
        Err(ProposerSetError::InvalidIdentity)
    );
}
