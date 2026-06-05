use super::*;
use l2_core::Hash32;

#[test]
fn signed_external_message_rejects_request_id_and_action_spoofing() {
    let message = signed_external("other-request", SignerAction::CommitBatch);
    let error = message
        .into_commit_batch(
            "expected-request",
            unix_time(),
            DEFAULT_SIGNER_MAX_BODY_BYTES,
        )
        .expect_err("request id spoofing");
    assert!(matches!(error, SignerValidationError::RequestIdMismatch));

    let message = signed_external("expected-request", SignerAction::FinalizeBatch);
    let error = message
        .into_commit_batch(
            "expected-request",
            unix_time(),
            DEFAULT_SIGNER_MAX_BODY_BYTES,
        )
        .expect_err("action spoofing");
    assert!(matches!(error, SignerValidationError::ActionMismatch));
}

#[test]
fn typed_sign_request_rejects_empty_commit_and_finalize_fields() {
    let mut commit = commit_request();
    commit.rollup_root_address.clear();
    let request = TypedSignRequest::commit_batch("commit-1".to_owned(), unix_time() + 300, commit);
    assert!(matches!(
        request.validate(unix_time()).unwrap_err(),
        SignerValidationError::InvalidCommitRequest
    ));

    let mut finalize = finalize_request();
    finalize.msg_value_nanoton = 0;
    let request =
        TypedSignRequest::finalize_batch("finalize-1".to_owned(), unix_time() + 300, finalize);
    assert!(matches!(
        request.validate(unix_time()).unwrap_err(),
        SignerValidationError::InvalidFinalizeRequest
    ));
}

fn signed_external(request_id: &str, action: SignerAction) -> SignedExternalMessage {
    SignedExternalMessage {
        request_id: request_id.to_owned(),
        action,
        boc_base64: "te6ccgEBAQEA".to_owned(),
        signer_address: "EQsequencer".to_owned(),
        valid_until: unix_time() + 300,
    }
}

fn commit_request() -> CommitBatchSignRequest {
    CommitBatchSignRequest {
        rollup_root_address: "EQroot".to_owned(),
        sender_address: "EQsequencer".to_owned(),
        msg_value_nanoton: 100_000_000,
        commitment: BatchCommitment {
            batch_no: 1,
            block_height: 0,
            block_hash: Hash32::new([1; 32]),
            roots_a: BatchRootsA {
                prev_state_root: Hash32::ZERO,
                state_root: Hash32::new([2; 32]),
                tx_root: Hash32::new([3; 32]),
            },
            roots_b: BatchRootsB {
                receipt_root: Hash32::new([4; 32]),
                withdrawal_root: Hash32::new([5; 32]),
                data_hash: Hash32::new([6; 32]),
            },
        },
    }
}

fn finalize_request() -> FinalizeBatchSignRequest {
    FinalizeBatchSignRequest {
        rollup_root_address: "EQroot".to_owned(),
        sender_address: "EQsequencer".to_owned(),
        batch_no: 1,
        msg_value_nanoton: 100_000_000,
    }
}
