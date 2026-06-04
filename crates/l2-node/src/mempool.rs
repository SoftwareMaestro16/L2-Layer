use async_trait::async_trait;
use l2_core::crypto::{decode_fixed, derive_account_id, verify_signature, Hash32};
use l2_core::{L2TransactionKind, SignedL2Transaction};
use redis::aio::ConnectionManager;
use serde_json::Error as SerdeError;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::config::NodeConfig;

const DEFAULT_REPLAY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_LEADER_TTL: Duration = Duration::from_secs(10);
const DEFAULT_REDIS_PREFIX: &str = "entropis:testnet";

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("wrong chain id")]
    WrongChainId,
    #[error("system deposit transactions are not accepted through the public mempool")]
    SystemTxNotAllowed,
    #[error("missing sender")]
    MissingSender,
    #[error("missing public key")]
    MissingPublicKey,
    #[error("missing signature")]
    MissingSignature,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("public key does not match sender")]
    PublicKeySenderMismatch,
    #[error("bad signature")]
    BadSignature,
    #[error("duplicate transaction {0}")]
    DuplicateTx(Hash32),
    #[error("nonce {nonce} for account {account_id} is locked")]
    NonceLocked { account_id: Hash32, nonce: u64 },
    #[error("mempool serialization failed: {0}")]
    Serialization(#[from] SerdeError),
    #[error("redis mempool failed: {0}")]
    Redis(#[from] redis::RedisError),
}

impl MempoolError {
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::DuplicateTx(_) | Self::NonceLocked { .. })
    }
}

#[async_trait]
pub trait MempoolStore: Send + Sync {
    async fn enqueue_validated(
        &self,
        tx: SignedL2Transaction,
        tx_hash: Hash32,
        account_id: Hash32,
        nonce: u64,
        ttl: Duration,
    ) -> Result<(), MempoolError>;

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError>;
    async fn acquire_leader_lock(&self, owner: &str, ttl: Duration) -> Result<bool, MempoolError>;
    async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError>;
}

pub type DynMempoolStore = Arc<dyn MempoolStore>;

#[derive(Clone)]
pub struct MempoolService {
    chain_id: String,
    replay_ttl: Duration,
    leader_ttl: Duration,
    store: DynMempoolStore,
}

impl MempoolService {
    pub fn new(chain_id: impl Into<String>, store: DynMempoolStore) -> Self {
        Self {
            chain_id: chain_id.into(),
            replay_ttl: DEFAULT_REPLAY_TTL,
            leader_ttl: DEFAULT_LEADER_TTL,
            store,
        }
    }

    pub async fn submit(&self, tx: SignedL2Transaction) -> Result<Hash32, MempoolError> {
        let (tx_hash, account_id, nonce) = self.validate_public_tx(&tx)?;
        self.store
            .enqueue_validated(tx, tx_hash, account_id, nonce, self.replay_ttl)
            .await?;
        Ok(tx_hash)
    }

    pub async fn pop_batch(
        &self,
        max_txs: usize,
    ) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        self.store.pop_batch(max_txs).await
    }

    pub async fn acquire_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        self.store.acquire_leader_lock(owner, self.leader_ttl).await
    }

    pub async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        self.store.release_leader_lock(owner).await
    }

    fn validate_public_tx(
        &self,
        tx: &SignedL2Transaction,
    ) -> Result<(Hash32, Hash32, u64), MempoolError> {
        if tx.chain_id != self.chain_id {
            return Err(MempoolError::WrongChainId);
        }
        if matches!(tx.kind, L2TransactionKind::Deposit { .. }) {
            return Err(MempoolError::SystemTxNotAllowed);
        }

        let from = tx.from.ok_or(MempoolError::MissingSender)?;
        let public_key_hex = tx
            .public_key
            .as_deref()
            .ok_or(MempoolError::MissingPublicKey)?;
        let signature_hex = tx
            .signature
            .as_deref()
            .ok_or(MempoolError::MissingSignature)?;
        let public_key =
            decode_fixed::<32>(public_key_hex).map_err(|_| MempoolError::InvalidPublicKey)?;
        if derive_account_id(&public_key) != from {
            return Err(MempoolError::PublicKeySenderMismatch);
        }
        if !verify_signature(public_key_hex, signature_hex, &tx.signing_payload()) {
            return Err(MempoolError::BadSignature);
        }

        Ok((tx.tx_hash(), from, tx.nonce))
    }
}

pub async fn build_mempool(config: &NodeConfig) -> Result<MempoolService, MempoolError> {
    let store = RedisMempoolStore::connect(config.redis_url.expose(), DEFAULT_REDIS_PREFIX).await?;
    Ok(MempoolService::new(
        config.chain_id.clone(),
        Arc::new(store),
    ))
}

#[derive(Debug, Default)]
pub struct MemoryMempoolStore {
    state: Mutex<MemoryMempoolState>,
}

#[derive(Debug, Default)]
struct MemoryMempoolState {
    queue: VecDeque<SignedL2Transaction>,
    queued_hashes: BTreeSet<Hash32>,
    replay: BTreeMap<Hash32, Instant>,
    nonce_locks: BTreeMap<(Hash32, u64), Instant>,
    leader_lock: Option<MemoryLeaderLock>,
}

#[derive(Debug)]
struct MemoryLeaderLock {
    owner: String,
    expires_at: Instant,
}

#[async_trait]
impl MempoolStore for MemoryMempoolStore {
    async fn enqueue_validated(
        &self,
        tx: SignedL2Transaction,
        tx_hash: Hash32,
        account_id: Hash32,
        nonce: u64,
        ttl: Duration,
    ) -> Result<(), MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();

        if state.replay.contains_key(&tx_hash) || state.queued_hashes.contains(&tx_hash) {
            return Err(MempoolError::DuplicateTx(tx_hash));
        }
        if state.nonce_locks.contains_key(&(account_id, nonce)) {
            return Err(MempoolError::NonceLocked { account_id, nonce });
        }

        let expires_at = Instant::now() + ttl;
        state.replay.insert(tx_hash, expires_at);
        state.nonce_locks.insert((account_id, nonce), expires_at);
        state.queued_hashes.insert(tx_hash);
        state.queue.push_back(tx);
        Ok(())
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        let mut state = self.state.lock().await;
        let mut txs = Vec::with_capacity(max_txs.min(state.queue.len()));
        for _ in 0..max_txs {
            let Some(tx) = state.queue.pop_front() else {
                break;
            };
            state.queued_hashes.remove(&tx.tx_hash());
            txs.push(tx);
        }
        Ok(txs)
    }

    async fn acquire_leader_lock(&self, owner: &str, ttl: Duration) -> Result<bool, MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();
        if state.leader_lock.is_some() {
            return Ok(false);
        }
        state.leader_lock = Some(MemoryLeaderLock {
            owner: owner.to_owned(),
            expires_at: Instant::now() + ttl,
        });
        Ok(true)
    }

    async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        let mut state = self.state.lock().await;
        let Some(lock) = state.leader_lock.as_ref() else {
            return Ok(false);
        };
        if lock.owner != owner {
            return Ok(false);
        }
        state.leader_lock = None;
        Ok(true)
    }
}

impl MemoryMempoolState {
    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.replay.retain(|_, expires_at| *expires_at > now);
        self.nonce_locks.retain(|_, expires_at| *expires_at > now);
        if self
            .leader_lock
            .as_ref()
            .is_some_and(|lock| lock.expires_at <= now)
        {
            self.leader_lock = None;
        }
    }
}

#[derive(Clone)]
pub struct RedisMempoolStore {
    connection: Arc<Mutex<ConnectionManager>>,
    prefix: String,
}

impl RedisMempoolStore {
    pub async fn connect(redis_url: &str, prefix: &str) -> Result<Self, MempoolError> {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_connection_manager().await?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            prefix: prefix.to_owned(),
        })
    }

    fn queue_key(&self) -> String {
        format!("{}:mempool:queue", self.prefix)
    }

    fn replay_key(&self, tx_hash: Hash32) -> String {
        format!("{}:mempool:replay:{}", self.prefix, tx_hash)
    }

    fn nonce_key(&self, account_id: Hash32, nonce: u64) -> String {
        format!("{}:mempool:nonce:{}:{}", self.prefix, account_id, nonce)
    }

    fn leader_key(&self) -> String {
        format!("{}:mempool:leader", self.prefix)
    }
}

#[async_trait]
impl MempoolStore for RedisMempoolStore {
    async fn enqueue_validated(
        &self,
        tx: SignedL2Transaction,
        tx_hash: Hash32,
        account_id: Hash32,
        nonce: u64,
        ttl: Duration,
    ) -> Result<(), MempoolError> {
        const ENQUEUE_SCRIPT: &str = r#"
            if redis.call('EXISTS', KEYS[1]) == 1 then
                return 1
            end
            if redis.call('EXISTS', KEYS[2]) == 1 then
                return 2
            end
            redis.call('SET', KEYS[1], '1', 'EX', ARGV[1])
            redis.call('SET', KEYS[2], '1', 'EX', ARGV[1])
            redis.call('RPUSH', KEYS[3], ARGV[2])
            return 0
        "#;

        let payload = serde_json::to_string(&tx)?;
        let mut connection = self.connection.lock().await;
        let result: i64 = redis::Script::new(ENQUEUE_SCRIPT)
            .key(self.replay_key(tx_hash))
            .key(self.nonce_key(account_id, nonce))
            .key(self.queue_key())
            .arg(ttl_secs(ttl))
            .arg(payload)
            .invoke_async(&mut *connection)
            .await?;

        match result {
            0 => Ok(()),
            1 => Err(MempoolError::DuplicateTx(tx_hash)),
            2 => Err(MempoolError::NonceLocked { account_id, nonce }),
            _ => Err(redis::RedisError::from((
                redis::ErrorKind::ResponseError,
                "unexpected redis enqueue script result",
            ))
            .into()),
        }
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        let mut connection = self.connection.lock().await;
        let mut out = Vec::with_capacity(max_txs);
        for _ in 0..max_txs {
            let payload: Option<String> = redis::cmd("LPOP")
                .arg(self.queue_key())
                .query_async(&mut *connection)
                .await?;
            let Some(payload) = payload else {
                break;
            };
            out.push(serde_json::from_str(&payload)?);
        }
        Ok(out)
    }

    async fn acquire_leader_lock(&self, owner: &str, ttl: Duration) -> Result<bool, MempoolError> {
        let mut connection = self.connection.lock().await;
        let result: Option<String> = redis::cmd("SET")
            .arg(self.leader_key())
            .arg(owner)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs(ttl))
            .query_async(&mut *connection)
            .await?;
        Ok(result.is_some())
    }

    async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        const RELEASE_SCRIPT: &str = r#"
            if redis.call('GET', KEYS[1]) == ARGV[1] then
                return redis.call('DEL', KEYS[1])
            end
            return 0
        "#;

        let mut connection = self.connection.lock().await;
        let released: i64 = redis::Script::new(RELEASE_SCRIPT)
            .key(self.leader_key())
            .arg(owner)
            .invoke_async(&mut *connection)
            .await?;
        Ok(released == 1)
    }
}

fn ttl_secs(ttl: Duration) -> u64 {
    ttl.as_secs().max(1)
}

#[cfg(test)]
#[path = "mempool_tests.rs"]
mod tests;
