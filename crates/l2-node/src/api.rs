use crate::config::NodeConfig;
use crate::faucet::{EntFaucetRequest, EntFaucetResponse, EntFaucetService, FaucetError};
use crate::mempool::{MempoolError, MempoolService};
use crate::storage::{DynStorage, StorageError};
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use l2_core::{
    crypto::sha256_bytes, DepositEvent, Hash32, L2Block, Sequencer, SequencerConfig,
    SignedL2Transaction, SubmitTxResponse,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    sequencer: Arc<RwLock<Sequencer>>,
    storage: DynStorage,
    mempool: MempoolService,
    ent_faucet: EntFaucetService,
    admin_auth: AdminAuth,
}

impl AppState {
    pub fn new(
        config: &NodeConfig,
        storage: DynStorage,
        mempool: MempoolService,
    ) -> Result<Self, FaucetError> {
        Ok(Self {
            sequencer: Arc::new(RwLock::new(Sequencer::new(SequencerConfig {
                chain_id: config.chain_id.clone(),
                ..SequencerConfig::default()
            }))),
            storage,
            mempool,
            ent_faucet: EntFaucetService::from_config(config)?,
            admin_auth: AdminAuth::new(Some(config.admin_token.expose().to_owned())),
        })
    }

    #[cfg(test)]
    fn test(admin_token: Option<&str>) -> Self {
        Self {
            sequencer: Arc::new(RwLock::new(Sequencer::new(SequencerConfig::default()))),
            storage: Arc::new(crate::storage::InMemoryStorage::default()),
            mempool: MempoolService::new(
                "entropis-testnet",
                Arc::new(crate::mempool::MemoryMempoolStore::default()),
            ),
            ent_faucet: EntFaucetService::from_config(&test_config()).expect("faucet config"),
            admin_auth: AdminAuth::new(admin_token.map(str::to_owned)),
        }
    }
}

pub async fn serve(
    config: NodeConfig,
    storage: DynStorage,
    mempool: MempoolService,
) -> anyhow::Result<()> {
    let state = AppState::new(&config, storage, mempool)?;
    spawn_block_producer(state.clone());

    let app = build_router(state);

    tracing::info!(addr = %config.node_addr, config = ?config, "starting l2 node");
    let listener = tokio::net::TcpListener::bind(config.node_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/tx", post(submit_tx))
        .route("/v1/tx/:hash", get(get_tx))
        .route("/v1/block/:height", get(get_block))
        .route("/v1/account/:id", get(get_account))
        .route("/v1/proof/withdrawal/:id", get(get_withdrawal_proof))
        .route("/v1/stream", get(stream))
        .route("/v1/admin/deposit", post(admin_deposit))
        .route("/v1/admin/faucet/ent", post(admin_ent_faucet))
        .route("/v1/admin/produce-block", post(admin_produce_block))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<SignedL2Transaction>,
) -> Result<Json<SubmitTxResponse>, ApiError> {
    let tx_hash = state.mempool.submit(tx).await?;
    Ok(Json(SubmitTxResponse { tx_hash }))
}

async fn admin_deposit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(deposit): Json<DepositEvent>,
) -> Result<StatusCode, ApiError> {
    state.admin_auth.authorize(&headers)?;
    validate_deposit_event(&deposit)?;

    let inserted = state.storage.save_deposit(deposit.clone()).await?;
    if inserted {
        let mut sequencer = state.sequencer.write().await;
        sequencer.ingest_deposits(vec![deposit]);
    }
    Ok(StatusCode::ACCEPTED)
}

async fn admin_produce_block(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    state.admin_auth.authorize(&headers)?;

    Ok(match produce_block_once(&state).await? {
        Some(block) => (StatusCode::CREATED, Json(block)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

async fn admin_ent_faucet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EntFaucetRequest>,
) -> Result<Json<EntFaucetResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let account_id = EntFaucetService::parse_account_id(&request.account_id)
        .map_err(|_| ApiError::bad_request("invalid account id"))?;
    let grant = state.ent_faucet.grant(&state.storage, account_id).await?;

    if let Some(deposit) = grant.deposit {
        let mut sequencer = state.sequencer.write().await;
        sequencer.ingest_deposits(vec![deposit]);
    }

    Ok(Json(grant.response))
}

async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    Ok(Json(account))
}

async fn get_block(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<impl IntoResponse, ApiError> {
    let block = state
        .storage
        .get_block(height)
        .await?
        .ok_or_else(|| ApiError::not_found("block not found"))?;
    Ok(Json(block))
}

async fn get_tx(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let hash = Hash32::from_hex(&hash).map_err(|_| ApiError::bad_request("invalid tx hash"))?;
    let transaction = state
        .storage
        .get_transaction(hash)
        .await?
        .ok_or_else(|| ApiError::not_found("transaction not found"))?;
    Ok(Json(transaction))
}

async fn get_withdrawal_proof(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid withdrawal id"))?;
    let proof = state
        .storage
        .get_withdrawal_proof(id)
        .await?
        .ok_or_else(|| ApiError::not_found("withdrawal proof not found"))?;
    Ok(Json(proof))
}

async fn stream(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        let _ = socket
            .send(Message::Text(
                "{\"type\":\"hello\",\"service\":\"ton-l2-rollup\"}".to_owned(),
            ))
            .await;
    })
}

fn spawn_block_producer(state: AppState) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(2)).await;
            if let Err(error) = produce_block_once(&state).await {
                tracing::error!(?error, "failed to produce l2 block");
            }
        }
    });
}

async fn produce_block_once(state: &AppState) -> Result<Option<L2Block>, ApiError> {
    const LEADER_OWNER: &str = "entropis-local-sequencer";
    if !state.mempool.acquire_leader_lock(LEADER_OWNER).await? {
        return Ok(None);
    }

    let timestamp = current_unix_time();
    let result = async {
        let queued = state.mempool.pop_batch(1024).await?;
        let mut sequencer = state.sequencer.write().await;
        for tx in queued {
            sequencer.submit_tx(tx);
        }
        Ok::<_, ApiError>(sequencer.produce_block(timestamp))
    }
    .await;
    let _ = state.mempool.release_leader_lock(LEADER_OWNER).await;

    let Some(block) = result? else {
        return Ok(None);
    };

    tracing::info!(
        height = block.header.height,
        state_root = %block.header.state_root,
        "produced l2 block"
    );
    state.storage.save_block(block.clone()).await?;
    Ok(Some(block))
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        tracing::error!(?error, "storage error");
        Self::internal("storage error")
    }
}

impl From<MempoolError> for ApiError {
    fn from(error: MempoolError) -> Self {
        if error.is_conflict() {
            return Self {
                status: StatusCode::CONFLICT,
                message: error.to_string(),
            };
        }
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
        }
    }
}

impl From<FaucetError> for ApiError {
    fn from(error: FaucetError) -> Self {
        match error {
            FaucetError::InvalidAccountId | FaucetError::ZeroAccountId => {
                Self::bad_request(error.to_string())
            }
            FaucetError::Storage(storage_error) => storage_error.into(),
            FaucetError::AmountOverflow => Self::internal(error.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[allow(dead_code)]
fn dev_deposit_id(seed: &str) -> Hash32 {
    sha256_bytes(seed.as_bytes())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AdminAuth {
    token: Option<String>,
}

impl AdminAuth {
    fn new(token: Option<String>) -> Self {
        Self {
            token: token.and_then(|token| {
                let token = token.trim().to_owned();
                (!token.is_empty()).then_some(token)
            }),
        }
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let Some(expected_token) = self.token.as_deref() else {
            return Err(ApiError::forbidden("admin api disabled"));
        };
        let Some(header_value) = headers.get(AUTHORIZATION) else {
            return Err(ApiError::unauthorized("missing admin bearer token"));
        };
        let header_value = header_value
            .to_str()
            .map_err(|_| ApiError::unauthorized("invalid authorization header"))?;
        let Some(actual_token) = header_value.strip_prefix("Bearer ") else {
            return Err(ApiError::unauthorized("missing admin bearer token"));
        };
        if !constant_time_eq(actual_token, expected_token) {
            return Err(ApiError::forbidden("invalid admin bearer token"));
        }

        Ok(())
    }
}

fn validate_deposit_event(deposit: &DepositEvent) -> Result<(), ApiError> {
    if deposit.deposit_id == Hash32::ZERO {
        return Err(ApiError::bad_request("deposit id must be non-zero"));
    }
    if deposit.recipient == Hash32::ZERO {
        return Err(ApiError::bad_request("recipient must be non-zero"));
    }
    if deposit.amount == 0 {
        return Err(ApiError::bad_request("amount must be non-zero"));
    }
    if deposit.l1_tx_hash == Hash32::ZERO {
        return Err(ApiError::bad_request("l1 tx hash must be non-zero"));
    }
    if deposit.l1_lt == 0 {
        return Err(ApiError::bad_request("l1 logical time must be non-zero"));
    }

    Ok(())
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();

    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        diff |= (left_byte ^ right_byte) as usize;
    }

    diff == 0
}

#[cfg(test)]
fn test_config() -> NodeConfig {
    use std::collections::BTreeMap;

    let env = BTreeMap::from([
        ("L2_NAME".to_owned(), "Entropis".to_owned()),
        ("L2_CHAIN_ID".to_owned(), "entropis-testnet".to_owned()),
        ("L2_NATIVE_TOKEN_NAME".to_owned(), "Entropis".to_owned()),
        ("L2_NATIVE_TOKEN_SYMBOL".to_owned(), "ENT".to_owned()),
        ("TON_NETWORK".to_owned(), "testnet".to_owned()),
        (
            "TONCENTER_V3_BASE_URL".to_owned(),
            "https://testnet.toncenter.com/api/v3".to_owned(),
        ),
        (
            "TONCENTER_API_KEY".to_owned(),
            "toncenter-secret-key".to_owned(),
        ),
        (
            "TONAPI_BASE_URL".to_owned(),
            "https://testnet.tonapi.io".to_owned(),
        ),
        ("TONAPI_KEY".to_owned(), "tonapi-secret-key".to_owned()),
        (
            "DATABASE_URL".to_owned(),
            "postgresql://user:pass@localhost:5432/l2".to_owned(),
        ),
        (
            "REDIS_URL".to_owned(),
            "redis://default:pass@localhost:6379".to_owned(),
        ),
        ("L2_ADMIN_TOKEN".to_owned(), "admin-secret-token".to_owned()),
        ("ENT_DECIMALS".to_owned(), "9".to_owned()),
        ("ENT_LOGO_PATH".to_owned(), "assets/entropis.png".to_owned()),
        ("ENT_FAUCET_REQUIRE_ADMIN".to_owned(), "true".to_owned()),
    ]);
    NodeConfig::from_lookup(|key| env.get(key).cloned()).expect("test config")
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
