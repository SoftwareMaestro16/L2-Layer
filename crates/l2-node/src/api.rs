use crate::config::NodeConfig;
use crate::da::{DataAvailabilityConfig, DynDa, StorageDaStore};
use crate::faucet::EntFaucetService;
use crate::mempool::MempoolService;
use crate::observability::{DynTonReadinessProbe, NodeMetrics, ToncenterReadinessClient};
use crate::storage::DynStorage;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use l2_core::{
    parse_l2_address, DepositEvent, Hash32, Sequencer, SequencerConfig, SignedL2Transaction,
    SubmitTxResponse,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod account;
mod auth;
mod challenge;
mod contract;
mod da;
mod error;
mod explorer;
mod faucet;
mod faucet_explorer;
mod mempool_ingress;
mod operator;
mod receipt;
mod sample;
mod stream;
#[cfg(test)]
mod test_support;
mod workers;

use account::get_account_metadata;
use auth::AdminAuth;
use challenge::{operator_observer_checkpoint, operator_observer_replay};
use contract::get_contract_state;
use da::{get_batch_da_payload, get_batch_da_payload_by_hash};
use error::ApiError;
use explorer::{
    admin_explorer_verifier_review, explorer_account, explorer_account_assets,
    explorer_account_code, explorer_account_transactions, explorer_blocks, explorer_code_source,
    explorer_deposit, explorer_deposits, explorer_summary, explorer_tx, explorer_verifier_submit,
    explorer_withdrawal, get_withdrawal_proof,
};
use faucet::{admin_ent_faucet, admin_ent_faucet_batch};
use faucet_explorer::explorer_faucet_batches;
use mempool_ingress::MempoolIngressGuard;
use operator::{
    healthz, operator_batch_finalizer, operator_batch_relayer, operator_failures, operator_metrics,
    readyz,
};
use receipt::{get_block_finality, get_receipt, get_tx_receipt};
use sample::post_contract_get_method;
use sample::{get_contract_method, get_sample_counter};
use stream::stream;
#[cfg(test)]
use test_support::test_config;
use workers::{
    produce_block_once, spawn_batch_finalizer, spawn_batch_relayer, spawn_block_producer,
    spawn_deposit_indexer,
};

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
    mempool_ingress: MempoolIngressGuard,
    dev_admin_deposits_enabled: bool,
    mempool_pop_batch_size: usize,
    tvm_adapter: l2_core::TvmAdapterMode,
    tvm_tonlib_library_path: Option<std::path::PathBuf>,
    tvm_getter_default_gas_limit: u64,
    tvm_getter_max_gas_limit: u64,
    tvm_getter_timeout_ms: u64,
    tvm_getter_max_stack_boc_bytes: usize,
}

impl AppState {
    pub async fn new(
        config: &NodeConfig,
        storage: DynStorage,
        mempool: MempoolService,
    ) -> anyhow::Result<Self> {
        let da = Arc::new(StorageDaStore::new(
            storage.clone(),
            DataAvailabilityConfig::from_node_config(config),
        ));
        let ton_readiness = Arc::new(ToncenterReadinessClient::from_config(config)?);
        let mut sequencer = Sequencer::new(SequencerConfig {
            chain_id: config.chain_id.clone(),
            gas_schedule: config.executor_gas_schedule,
            max_internal_queue_len: config.internal_queue_max_len,
            max_internal_messages_per_block: config.internal_queue_max_per_block,
            internal_message_gas_limit: config.internal_message_gas_limit,
            tvm_adapter_mode: config.tvm_adapter.clone(),
            tvm_tonlib_library_path: config.tvm_tonlib_library_path.clone(),
            ..SequencerConfig::default()
        });
        if let Some(snapshot) = storage.latest_internal_queue_snapshot().await? {
            sequencer.restore_internal_queue(snapshot.queue)?;
        }
        Ok(Self {
            sequencer: Arc::new(RwLock::new(sequencer)),
            storage,
            da,
            mempool,
            metrics: Arc::new(NodeMetrics::default()),
            ton_readiness,
            ent_faucet: EntFaucetService::from_config(config)?,
            admin_auth: AdminAuth::new(Some(config.admin_token.expose().to_owned())),
            mempool_ingress: MempoolIngressGuard::from_config(config),
            dev_admin_deposits_enabled: config.dev_admin_deposits_enabled,
            mempool_pop_batch_size: config.mempool_pop_batch_size,
            tvm_adapter: config.tvm_adapter.clone(),
            tvm_tonlib_library_path: config.tvm_tonlib_library_path.clone(),
            tvm_getter_default_gas_limit: config.tvm_getter_default_gas_limit,
            tvm_getter_max_gas_limit: config.tvm_getter_max_gas_limit,
            tvm_getter_timeout_ms: config.tvm_getter_timeout_ms,
            tvm_getter_max_stack_boc_bytes: config.tvm_getter_max_stack_boc_bytes,
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
            mempool_ingress: MempoolIngressGuard::from_config(&test_config()),
            dev_admin_deposits_enabled: true,
            mempool_pop_batch_size: 1024,
            tvm_adapter: l2_core::TvmAdapterMode::Prototype,
            tvm_tonlib_library_path: None,
            tvm_getter_default_gas_limit: l2_core::DEFAULT_GETTER_GAS_LIMIT,
            tvm_getter_max_gas_limit: 1_000_000,
            tvm_getter_timeout_ms: 500,
            tvm_getter_max_stack_boc_bytes: l2_core::DEFAULT_MAX_GETTER_STACK_BOC_BYTES,
        }
    }
}

pub async fn serve(
    config: NodeConfig,
    storage: DynStorage,
    mempool: MempoolService,
) -> anyhow::Result<()> {
    let state = AppState::new(&config, storage, mempool).await?;
    spawn_block_producer(state.clone());
    spawn_deposit_indexer(&config, state.clone());
    spawn_batch_relayer(
        &config,
        state.storage.clone(),
        state.da.clone(),
        state.metrics.clone(),
    );
    spawn_batch_finalizer(&config, state.storage.clone(), state.metrics.clone());

    let app = build_router(state);

    tracing::info!(addr = %config.node_addr, config = ?config, "starting l2 node");
    let listener = tokio::net::TcpListener::bind(config.node_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/tx", post(submit_tx))
        .route("/v1/tx/:hash", get(get_tx))
        .route("/v1/tx/:hash/receipt", get(get_tx_receipt))
        .route("/v1/receipt/:hash", get(get_receipt))
        .route("/v1/block/:height", get(get_block))
        .route("/v1/block/:height/finality", get(get_block_finality))
        .route("/v1/account/:id", get(get_account))
        .route("/v1/account/:id/metadata", get(get_account_metadata))
        .route("/v1/sample-counter/:id", get(get_sample_counter))
        .route("/v1/contract/:id/state", get(get_contract_state))
        .route(
            "/v1/contract/:id/get-method",
            post(post_contract_get_method),
        )
        .route("/v1/contract/:id/get/:method", get(get_contract_method))
        .route("/v1/da/batch/:height", get(get_batch_da_payload))
        .route(
            "/v1/da/batch/:height/:data_hash",
            get(get_batch_da_payload_by_hash),
        )
        .route("/v1/mempool/metrics", get(get_mempool_metrics))
        .route("/v1/operator/metrics", get(operator_metrics))
        .route("/v1/operator/failures", get(operator_failures))
        .route("/v1/operator/batch-relayer", get(operator_batch_relayer))
        .route(
            "/v1/operator/batch-finalizer",
            get(operator_batch_finalizer),
        )
        .route(
            "/v1/operator/observer/checkpoint",
            get(operator_observer_checkpoint),
        )
        .route(
            "/v1/operator/observer/replay",
            post(operator_observer_replay),
        )
        .route("/v1/explorer/summary", get(explorer_summary))
        .route("/v1/explorer/blocks", get(explorer_blocks))
        .route("/v1/explorer/deposits", get(explorer_deposits))
        .route("/v1/explorer/faucet/batches", get(explorer_faucet_batches))
        .route("/v1/explorer/account/:id", get(explorer_account))
        .route(
            "/v1/explorer/account/:id/assets",
            get(explorer_account_assets),
        )
        .route("/v1/explorer/account/:id/code", get(explorer_account_code))
        .route(
            "/v1/explorer/account/:id/transactions",
            get(explorer_account_transactions),
        )
        .route("/v1/explorer/tx/:hash", get(explorer_tx))
        .route(
            "/v1/explorer/code/:code_hash/source",
            get(explorer_code_source),
        )
        .route(
            "/v1/explorer/verifier/submissions",
            post(explorer_verifier_submit),
        )
        .route(
            "/v1/admin/explorer/verifier/submissions/:submission_id/review",
            post(admin_review_verifier_submission),
        )
        .route("/v1/explorer/deposit/:id", get(explorer_deposit))
        .route("/v1/explorer/withdrawal/:id", get(explorer_withdrawal))
        .route("/v1/proof/withdrawal/:id", get(get_withdrawal_proof))
        .route("/v1/stream", get(stream))
        .route("/v1/admin/deposit", post(admin_deposit))
        .route("/v1/admin/faucet/ent", post(admin_ent_faucet))
        .route("/v1/admin/faucet/ent/batch", post(admin_ent_faucet_batch))
        .route("/v1/admin/produce-block", post(admin_produce_block))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn submit_tx(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(tx): Json<SignedL2Transaction>,
) -> Result<Json<SubmitTxResponse>, ApiError> {
    if let Err(error) = state.mempool_ingress.check(peer).await {
        state
            .mempool
            .record_external_rejection(error.reason_code())
            .await;
        return Err(error.into());
    }
    let tx_hash = state.mempool.submit(tx).await?;
    Ok(Json(SubmitTxResponse { tx_hash }))
}

async fn admin_deposit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(deposit): Json<DepositEvent>,
) -> Result<StatusCode, ApiError> {
    state.admin_auth.authorize(&headers)?;
    if !state.dev_admin_deposits_enabled {
        return Err(ApiError::forbidden("dev admin deposits disabled"));
    }
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

async fn admin_review_verifier_submission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(submission_id): Path<String>,
    Json(request): Json<explorer::account::VerifierReviewRequest>,
) -> Result<impl IntoResponse, ApiError> {
    state.admin_auth.authorize(&headers)?;
    admin_explorer_verifier_review(State(state), Path(submission_id), Json(request)).await
}

async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
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

async fn get_mempool_metrics(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(state.mempool.metrics().await?))
}

fn validate_deposit_event(deposit: &DepositEvent) -> Result<(), ApiError> {
    if deposit.deposit_id == Hash32::ZERO {
        return Err(ApiError::bad_request("deposit id must be non-zero"));
    }
    if deposit.recipient == Hash32::ZERO {
        return Err(ApiError::bad_request("reserved zero address"));
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

#[cfg(test)]
#[path = "api_contract_tests.rs"]
mod contract_tests;
#[cfg(test)]
#[path = "api_explorer_tests.rs"]
mod explorer_tests;
#[cfg(test)]
#[path = "api_operator_tests.rs"]
mod operator_tests;
#[cfg(test)]
#[path = "api_sample_tests.rs"]
mod sample_tests;
#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
