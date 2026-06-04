use crate::config::NodeConfig;
use crate::da::{DataAvailabilityConfig, DynDa, StorageDaStore};
#[cfg(test)]
use crate::faucet::EntFaucetRequest;
use crate::faucet::EntFaucetService;
use crate::mempool::MempoolService;
use crate::observability::{DynTonReadinessProbe, NodeMetrics, ToncenterReadinessClient};
use crate::storage::DynStorage;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::{Path, State};
#[cfg(test)]
use axum::http::HeaderMap;
#[cfg(test)]
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
#[cfg(test)]
use l2_core::{crypto::sha256_bytes, DepositEvent};
use l2_core::{Hash32, Sequencer, SequencerConfig, SignedL2Transaction, SubmitTxResponse};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod admin;
mod auth;
mod error;
mod operator;
#[cfg(test)]
mod test_support;
mod workers;

#[cfg(test)]
use admin::validate_deposit_event;
use admin::{admin_deposit, admin_ent_faucet, admin_produce_block};
use auth::AdminAuth;
use error::ApiError;
use operator::{healthz, operator_batch_commits, operator_failures, operator_metrics, readyz};
#[cfg(test)]
use test_support::test_config;
#[cfg(test)]
use workers::produce_block_once;
use workers::{spawn_batch_relayer, spawn_block_producer, spawn_deposit_indexer};

#[derive(Clone)]
pub struct AppState {
    sequencer: Arc<RwLock<Sequencer>>,
    storage: DynStorage,
    da: DynDa,
    mempool: MempoolService,
    metrics: Arc<NodeMetrics>,
    ton_readiness: DynTonReadinessProbe,
    ent_faucet: EntFaucetService,
    admin_auth: AdminAuth,
    dev_admin_deposits_enabled: bool,
    mempool_pop_batch_size: usize,
}

impl AppState {
    pub fn new(
        config: &NodeConfig,
        storage: DynStorage,
        mempool: MempoolService,
    ) -> anyhow::Result<Self> {
        let da = Arc::new(StorageDaStore::new(
            storage.clone(),
            DataAvailabilityConfig::from_node_config(config),
        ));
        let ton_readiness = Arc::new(ToncenterReadinessClient::from_config(config)?);
        Ok(Self {
            sequencer: Arc::new(RwLock::new(Sequencer::new(SequencerConfig {
                chain_id: config.chain_id.clone(),
                gas_schedule: config.executor_gas_schedule,
                ..SequencerConfig::default()
            }))),
            storage,
            da,
            mempool,
            metrics: Arc::new(NodeMetrics::default()),
            ton_readiness,
            ent_faucet: EntFaucetService::from_config(config)?,
            admin_auth: AdminAuth::new(Some(config.admin_token.expose().to_owned())),
            dev_admin_deposits_enabled: config.dev_admin_deposits_enabled,
            mempool_pop_batch_size: config.mempool_pop_batch_size,
        })
    }

    #[cfg(test)]
    fn test(admin_token: Option<&str>) -> Self {
        let storage: DynStorage = Arc::new(crate::storage::InMemoryStorage::default());
        let da = Arc::new(StorageDaStore::new(
            storage.clone(),
            DataAvailabilityConfig::from_node_config(&test_config()),
        ));
        Self {
            sequencer: Arc::new(RwLock::new(Sequencer::new(SequencerConfig::default()))),
            storage,
            da,
            mempool: MempoolService::new(
                "entropis-testnet",
                Arc::new(crate::mempool::MemoryMempoolStore::default()),
            ),
            metrics: Arc::new(NodeMetrics::default()),
            ton_readiness: Arc::new(crate::observability::ReadyTonReadinessProbe),
            ent_faucet: EntFaucetService::from_config(&test_config()).expect("faucet config"),
            admin_auth: AdminAuth::new(admin_token.map(str::to_owned)),
            dev_admin_deposits_enabled: true,
            mempool_pop_batch_size: 1024,
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
    spawn_deposit_indexer(&config, state.clone());
    spawn_batch_relayer(
        &config,
        state.storage.clone(),
        state.da.clone(),
        state.metrics.clone(),
    );

    let app = build_router(state);

    tracing::info!(
        addr = %config.node_addr,
        startup = ?config.startup_summary(),
        "starting l2 node"
    );
    let listener = tokio::net::TcpListener::bind(config.node_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/tx", post(submit_tx))
        .route("/v1/tx/:hash", get(get_tx))
        .route("/v1/block/:height", get(get_block))
        .route("/v1/account/:id", get(get_account))
        .route("/v1/mempool/metrics", get(get_mempool_metrics))
        .route("/v1/operator/metrics", get(operator_metrics))
        .route("/v1/operator/batch-commits", get(operator_batch_commits))
        .route("/v1/operator/failures", get(operator_failures))
        .route("/v1/proof/withdrawal/:id", get(get_withdrawal_proof))
        .route("/v1/stream", get(stream))
        .route("/v1/admin/deposit", post(admin_deposit))
        .route("/v1/admin/faucet/ent", post(admin_ent_faucet))
        .route("/v1/admin/produce-block", post(admin_produce_block))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<SignedL2Transaction>,
) -> Result<Json<SubmitTxResponse>, ApiError> {
    let tx_hash = state.mempool.submit(tx).await?;
    Ok(Json(SubmitTxResponse { tx_hash }))
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

async fn get_mempool_metrics(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(state.mempool.metrics().await?))
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

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
