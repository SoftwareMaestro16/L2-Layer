use anyhow::Context;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use l2_core::{
    crypto::sha256_bytes, DepositEvent, Hash32, L2Block, Sequencer, SequencerConfig,
    SignedL2Transaction, SubmitTxResponse,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    sequencer: Arc<RwLock<Sequencer>>,
    blocks: Arc<RwLock<Vec<L2Block>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "l2_node=info,tower_http=info".to_owned()),
        )
        .init();

    let state = AppState {
        sequencer: Arc::new(RwLock::new(Sequencer::new(SequencerConfig::default()))),
        blocks: Arc::new(RwLock::new(Vec::new())),
    };

    spawn_block_producer(state.clone());

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/tx", post(submit_tx))
        .route("/v1/tx/:hash", get(get_tx))
        .route("/v1/block/:height", get(get_block))
        .route("/v1/account/:id", get(get_account))
        .route("/v1/proof/withdrawal/:id", get(get_withdrawal_proof))
        .route("/v1/stream", get(stream))
        .route("/v1/admin/deposit", post(admin_deposit))
        .route("/v1/admin/produce-block", post(admin_produce_block))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("L2_NODE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()
        .context("invalid L2_NODE_ADDR")?;

    tracing::info!(%addr, "starting l2 node");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<SignedL2Transaction>,
) -> Json<SubmitTxResponse> {
    let mut sequencer = state.sequencer.write().await;
    let tx_hash = sequencer.submit_tx(tx);
    Json(SubmitTxResponse { tx_hash })
}

async fn admin_deposit(
    State(state): State<AppState>,
    Json(deposit): Json<DepositEvent>,
) -> StatusCode {
    let mut sequencer = state.sequencer.write().await;
    sequencer.ingest_deposits(vec![deposit]);
    StatusCode::ACCEPTED
}

async fn admin_produce_block(State(state): State<AppState>) -> impl IntoResponse {
    match produce_block_once(&state).await {
        Some(block) => (StatusCode::CREATED, Json(block)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
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
    let blocks = state.blocks.read().await;
    let block = blocks
        .iter()
        .find(|block| block.header.height == height)
        .cloned()
        .ok_or_else(|| ApiError::not_found("block not found"))?;
    Ok(Json(block))
}

async fn get_tx(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let hash = Hash32::from_hex(&hash).map_err(|_| ApiError::bad_request("invalid tx hash"))?;
    let blocks = state.blocks.read().await;
    for block in blocks.iter() {
        if let Some((index, tx)) = block
            .transactions
            .iter()
            .enumerate()
            .find(|(_, tx)| tx.tx_hash() == hash)
        {
            return Ok(Json(TxLookup {
                block_height: block.header.height,
                transaction: tx.clone(),
                receipt: block.receipts.get(index).cloned(),
            }));
        }
    }
    Err(ApiError::not_found("transaction not found"))
}

async fn get_withdrawal_proof(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid withdrawal id"))?;
    let blocks = state.blocks.read().await;
    for block in blocks.iter() {
        if let Some(proof) = block.withdrawal_proof(id) {
            return Ok(Json(proof));
        }
    }
    Err(ApiError::not_found("withdrawal proof not found"))
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
            let _ = produce_block_once(&state).await;
        }
    });
}

async fn produce_block_once(state: &AppState) -> Option<L2Block> {
    let timestamp = current_unix_time();
    let block = {
        let mut sequencer = state.sequencer.write().await;
        sequencer.produce_block(timestamp)
    }?;

    tracing::info!(
        height = block.header.height,
        state_root = %block.header.state_root,
        "produced l2 block"
    );
    state.blocks.write().await.push(block.clone());
    Some(block)
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[derive(Serialize)]
struct TxLookup {
    block_height: u64,
    transaction: SignedL2Transaction,
    receipt: Option<l2_core::Receipt>,
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
