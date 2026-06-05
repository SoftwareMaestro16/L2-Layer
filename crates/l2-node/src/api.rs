use crate::config::NodeConfig;
use crate::da::{DataAvailabilityConfig, DynDa, StorageDaStore};
use crate::faucet::{EntFaucetRequest, EntFaucetResponse, EntFaucetService};
use crate::finalizer::{BatchFinalizer, BatchFinalizerConfig};
use crate::indexer::{DepositIndexerConfig, TonDepositIndexer, ToncenterClient};
use crate::mempool::MempoolService;
use crate::observability::{DynTonReadinessProbe, NodeMetrics, ToncenterReadinessClient};
use crate::relayer::{BatchRelayer, BatchRelayerConfig, ToncenterCommitProvider};
use crate::signer::{RemoteCommitBatchSigner, RemoteFinalizeBatchSigner};
use crate::storage::DynStorage;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use l2_core::{
    parse_l2_address, DepositEvent, Hash32, L2Block, Sequencer, SequencerConfig,
    SignedL2Transaction, SubmitTxResponse,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration, Instant};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

mod auth;
mod challenge;
mod da;
mod error;
mod explorer;
mod operator;
mod sample;
mod stream;
#[cfg(test)]
mod test_support;

use auth::AdminAuth;
use challenge::{operator_observer_checkpoint, operator_observer_replay};
use da::{get_batch_da_payload, get_batch_da_payload_by_hash};
use error::ApiError;
use explorer::{
    explorer_account, explorer_account_transactions, explorer_blocks, explorer_deposit,
    explorer_deposits, explorer_summary, explorer_tx, explorer_withdrawal, get_withdrawal_proof,
};
use operator::{
    healthz, operator_batch_finalizer, operator_batch_relayer, operator_failures, operator_metrics,
    readyz,
};
use sample::{get_contract_method, get_sample_counter};
use stream::stream;
#[cfg(test)]
use test_support::test_config;

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
    spawn_batch_finalizer(&config, state.storage.clone(), state.metrics.clone());

    let app = build_router(state);

    tracing::info!(addr = %config.node_addr, config = ?config, "starting l2 node");
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
        .route("/v1/sample-counter/:id", get(get_sample_counter))
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
        .route("/v1/explorer/account/:id", get(explorer_account))
        .route(
            "/v1/explorer/account/:id/transactions",
            get(explorer_account_transactions),
        )
        .route("/v1/explorer/tx/:hash", get(explorer_tx))
        .route("/v1/explorer/deposit/:id", get(explorer_deposit))
        .route("/v1/explorer/withdrawal/:id", get(explorer_withdrawal))
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

fn spawn_deposit_indexer(config: &NodeConfig, state: AppState) {
    let Some(indexer_config) = DepositIndexerConfig::from_node_config(config) else {
        return;
    };
    let poll_interval = Duration::from_millis(config.l1_deposit_poll_interval_ms);
    let indexer = TonDepositIndexer::new(indexer_config, ToncenterClient::from_config(config));
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match indexer.poll_once(&state.storage, &state.sequencer).await {
                Ok(stats) => {
                    state.metrics.record_indexer_poll(
                        stats.fetched,
                        stats.accepted,
                        stats.duplicates,
                    );
                    tracing::info!(
                        fetched = stats.fetched,
                        accepted = stats.accepted,
                        duplicates = stats.duplicates,
                        "ton deposit indexer poll completed"
                    );
                }
                Err(error) => {
                    state.metrics.record_indexer_error();
                    tracing::warn!(?error, "ton deposit indexer poll failed");
                }
            }
        }
    });
}

fn spawn_batch_relayer(
    config: &NodeConfig,
    storage: DynStorage,
    da: DynDa,
    metrics: Arc<NodeMetrics>,
) {
    let Some(relayer_config) = BatchRelayerConfig::from_node_config(config) else {
        return;
    };
    let Some(signer) = RemoteCommitBatchSigner::from_config(config) else {
        tracing::error!("batch relayer enabled without signer config");
        return;
    };
    let poll_interval = Duration::from_millis(relayer_config.poll_interval_ms);
    let retry_backoff = Duration::from_millis(relayer_config.retry_backoff_ms);
    let relayer = BatchRelayer::new(
        relayer_config,
        storage,
        da,
        signer,
        ToncenterCommitProvider::from_config(config),
    );
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match relayer.relay_once().await {
                Ok(stats) => {
                    metrics.record_relayer_poll(
                        stats.considered,
                        stats.submitted,
                        stats.confirmed,
                        stats.failed,
                        stats.skipped,
                    );
                    tracing::info!(
                        considered = stats.considered,
                        submitted = stats.submitted,
                        confirmed = stats.confirmed,
                        failed = stats.failed,
                        skipped = stats.skipped,
                        "batch relayer poll completed"
                    );
                }
                Err(error) => {
                    metrics.record_relayer_error();
                    tracing::warn!(?error, "batch relayer poll failed");
                    sleep(retry_backoff).await;
                }
            }
        }
    });
}

fn spawn_batch_finalizer(config: &NodeConfig, storage: DynStorage, metrics: Arc<NodeMetrics>) {
    let Some(finalizer_config) = BatchFinalizerConfig::from_node_config(config) else {
        return;
    };
    let Some(signer) = RemoteFinalizeBatchSigner::from_config(config) else {
        tracing::error!("batch finalizer enabled without finalize signer config");
        return;
    };
    let poll_interval = Duration::from_millis(finalizer_config.poll_interval_ms);
    let retry_backoff = Duration::from_millis(finalizer_config.retry_backoff_ms);
    let finalizer = BatchFinalizer::new(
        finalizer_config,
        storage,
        signer,
        ToncenterCommitProvider::from_config(config),
    );
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match finalizer.finalize_once().await {
                Ok(stats) => {
                    metrics.record_finalizer_poll(
                        stats.created_pending,
                        stats.considered,
                        stats.submitted,
                        stats.finalized,
                        stats.failed,
                        stats.waiting,
                        stats.skipped,
                    );
                    tracing::info!(
                        created_pending = stats.created_pending,
                        considered = stats.considered,
                        submitted = stats.submitted,
                        finalized = stats.finalized,
                        failed = stats.failed,
                        waiting = stats.waiting,
                        skipped = stats.skipped,
                        "batch finalizer poll completed"
                    );
                }
                Err(error) => {
                    metrics.record_finalizer_error();
                    tracing::warn!(?error, "batch finalizer poll failed");
                    sleep(retry_backoff).await;
                }
            }
        }
    });
}

async fn produce_block_once(state: &AppState) -> Result<Option<L2Block>, ApiError> {
    const LEADER_OWNER: &str = "entropis-local-sequencer";
    state.metrics.record_block_attempt();
    if !state.mempool.acquire_leader_lock(LEADER_OWNER).await? {
        state.metrics.record_empty_block();
        return Ok(None);
    }

    let timestamp = current_unix_time();
    let result = async {
        let queued = state
            .mempool
            .pop_batch(state.mempool_pop_batch_size)
            .await?;
        let mut sequencer = state.sequencer.write().await;
        let mut candidate = sequencer.clone();
        for tx in queued.iter().cloned() {
            candidate.submit_tx(tx);
        }

        let Some(block) = candidate.produce_block(timestamp) else {
            return Ok(None);
        };

        tracing::info!(
            height = block.header.height,
            state_root = %block.header.state_root,
            "produced l2 block"
        );
        let da_started = Instant::now();
        let da_ref = match state.da.write_batch_payload(&block).await {
            Ok(da_ref) => da_ref,
            Err(error) => {
                for tx in queued {
                    sequencer.submit_tx(tx);
                }
                return Err(error.into());
            }
        };
        state.metrics.record_da_write_latency(da_started.elapsed());
        tracing::info!(
            height = da_ref.block_height,
            data_hash = %da_ref.data_hash,
            payload_bytes = da_ref.payload_size,
            "published l2 batch data"
        );
        let storage_started = Instant::now();
        if let Err(error) = state.storage.save_block(block.clone()).await {
            for tx in queued {
                sequencer.submit_tx(tx);
            }
            return Err(error.into());
        }
        state
            .metrics
            .record_storage_save_block_latency(storage_started.elapsed());
        *sequencer = candidate;
        Ok(Some(block))
    }
    .await;
    let _ = state.mempool.release_leader_lock(LEADER_OWNER).await;

    match result {
        Ok(Some(block)) => {
            state.metrics.record_block_produced(block.header.height);
            Ok(Some(block))
        }
        Ok(None) => {
            state.metrics.record_empty_block();
            Ok(None)
        }
        Err(error) => {
            state.metrics.record_block_error();
            Err(error)
        }
    }
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
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
