use super::*;
use async_trait::async_trait;
use axum::http::StatusCode;
use l2_core::Hash32;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn remote_client_accepts_valid_signer_response() {
    let backend = MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc()));
    let endpoint = spawn_signer(backend, 16 * 1024, 10).await;
    let signer = RemoteCommitBatchSigner::new(
        format!("{endpoint}/sign-commit"),
        crate::config::SecretString::new("test-signer-token".to_owned()).unwrap(),
    );

    let signed = signer
        .sign_commit_batch(commit_request())
        .await
        .expect("signed commit");

    assert_eq!(signed.signer_address, "EQsequencer");
    assert_eq!(signed.boc_base64, valid_boc());
}

#[tokio::test]
async fn signer_service_rejects_missing_or_wrong_bearer_token() {
    let endpoint = spawn_signer(
        MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc())),
        16 * 1024,
        10,
    )
    .await;
    let client = reqwest::Client::new();

    let missing = client
        .post(format!("{endpoint}/sign-commit"))
        .json(&typed_request())
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let wrong = client
        .post(format!("{endpoint}/sign-commit"))
        .bearer_auth("wrong-token")
        .json(&typed_request())
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signer_service_rejects_oversized_payload() {
    let endpoint = spawn_signer(
        MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc())),
        128,
        10,
    )
    .await;
    let response = reqwest::Client::new()
        .post(format!("{endpoint}/sign-commit"))
        .bearer_auth("test-signer-token")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body("x".repeat(1024))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn signer_service_rejects_known_unsupported_action() {
    let endpoint = spawn_signer(
        MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc())),
        16 * 1024,
        10,
    )
    .await;
    let request = TypedSignRequest {
        request_id: "finalize-1".to_owned(),
        role: SignerRole::Sequencer,
        valid_until: unix_time() + 300,
        action: TypedSignAction::FinalizeBatch(FinalizeBatchSignRequest {
            rollup_root_address: "EQroot".to_owned(),
            sender_address: "EQsequencer".to_owned(),
            batch_no: 1,
            msg_value_nanoton: 100_000_000,
        }),
    };

    let response = reqwest::Client::new()
        .post(format!("{endpoint}/sign"))
        .bearer_auth("test-signer-token")
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "unsupported_action");
}

#[tokio::test]
async fn signer_service_rejects_expired_request() {
    let endpoint = spawn_signer(
        MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc())),
        16 * 1024,
        10,
    )
    .await;
    let mut request = typed_request();
    request.valid_until = unix_time().saturating_sub(1);

    let response = reqwest::Client::new()
        .post(format!("{endpoint}/sign-commit"))
        .bearer_auth("test-signer-token")
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["error"], "expired_request");
}

#[tokio::test]
async fn signer_service_rate_limits_requests() {
    let endpoint = spawn_signer(
        MockBackend::new(signed("EQsequencer", unix_time() + 300, valid_boc())),
        16 * 1024,
        1,
    )
    .await;
    let client = reqwest::Client::new();

    let first = client
        .post(format!("{endpoint}/sign-commit"))
        .bearer_auth("test-signer-token")
        .json(&typed_request())
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = client
        .post(format!("{endpoint}/sign-commit"))
        .bearer_auth("test-signer-token")
        .json(&typed_request())
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn remote_client_rejects_expired_response() {
    let backend = MockBackend::new(signed(
        "EQsequencer",
        unix_time().saturating_sub(1),
        valid_boc(),
    ));
    let endpoint = spawn_signer(backend, 16 * 1024, 10).await;
    let signer = RemoteCommitBatchSigner::new(
        format!("{endpoint}/sign-commit"),
        crate::config::SecretString::new("test-signer-token".to_owned()).unwrap(),
    );

    let error = signer
        .sign_commit_batch(commit_request())
        .await
        .expect_err("expired response");

    assert_eq!(error.safe_code(), "signer_rejected");
}

#[tokio::test]
async fn remote_client_rejects_malformed_boc_response() {
    let backend = MockBackend::new(signed(
        "EQsequencer",
        unix_time() + 300,
        "***not-base64***".to_owned(),
    ));
    let endpoint = spawn_signer(backend, 16 * 1024, 10).await;
    let signer = RemoteCommitBatchSigner::new(
        format!("{endpoint}/sign-commit"),
        crate::config::SecretString::new("test-signer-token".to_owned()).unwrap(),
    );

    let error = signer
        .sign_commit_batch(commit_request())
        .await
        .expect_err("malformed boc");

    assert_eq!(error.safe_code(), "signer_rejected");
}

#[test]
fn signer_service_config_debug_redacts_token() {
    let config = SignerServiceConfig {
        token: crate::config::SecretString::new("very-secret-signer-token".to_owned()).unwrap(),
        signer_address: "EQsequencer".to_owned(),
        role: SignerRole::Sequencer,
        max_body_bytes: 16 * 1024,
        rate_limit_per_minute: 60,
    };

    let debug = format!("{config:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("very-secret-signer-token"));
}

#[derive(Clone)]
struct MockBackend {
    response: SignedCommitBatch,
    requests: Arc<Mutex<Vec<TypedSignRequest>>>,
}

impl MockBackend {
    fn new(response: SignedCommitBatch) -> Self {
        Self {
            response,
            requests: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl TypedSignerBackend for MockBackend {
    async fn sign(
        &self,
        request: TypedSignRequest,
    ) -> Result<SignedCommitBatch, SignerBackendError> {
        self.requests.lock().await.push(request);
        Ok(self.response.clone())
    }
}

async fn spawn_signer(backend: MockBackend, max_body_bytes: usize, rate_limit: u32) -> String {
    let router = build_signer_router(
        SignerServiceConfig {
            token: crate::config::SecretString::new("test-signer-token".to_owned()).unwrap(),
            signer_address: "EQsequencer".to_owned(),
            role: SignerRole::Sequencer,
            max_body_bytes,
            rate_limit_per_minute: rate_limit,
        },
        backend,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

fn typed_request() -> TypedSignRequest {
    TypedSignRequest::commit_batch(
        "commit-batch-1-test".to_owned(),
        unix_time() + 300,
        commit_request(),
    )
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

fn signed(signer_address: &str, valid_until: u64, boc_base64: String) -> SignedCommitBatch {
    SignedCommitBatch {
        boc_base64,
        signer_address: signer_address.to_owned(),
        valid_until,
    }
}

fn valid_boc() -> String {
    "te6ccgEBAQEA".to_owned()
}
