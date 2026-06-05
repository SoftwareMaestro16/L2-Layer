use async_trait::async_trait;
use l2_core::{Hash32, SignedL2Transaction};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::error::MempoolError;
use super::types::{
    fair_order_indices, MempoolStore, MempoolStoreLimits, MempoolStoreStats, QueuedMempoolTx,
    ValidatedMempoolTx,
};

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

    fn account_queue_key(&self, account_id: Hash32) -> String {
        format!("{}:mempool:account-queue:{}", self.prefix, account_id)
    }

    fn account_nonce_key(&self, account_id: Hash32) -> String {
        format!("{}:mempool:account-nonces:{}", self.prefix, account_id)
    }

    fn rate_key(&self, account_id: Hash32) -> String {
        format!("{}:mempool:rate:{}", self.prefix, account_id)
    }

    fn leader_key(&self) -> String {
        format!("{}:mempool:leader", self.prefix)
    }

    fn evicted_key(&self) -> String {
        format!("{}:mempool:evicted", self.prefix)
    }
}

#[async_trait]
impl MempoolStore for RedisMempoolStore {
    async fn consume_rate_limit(
        &self,
        account_id: Hash32,
        window: Duration,
        max_submissions: u32,
    ) -> Result<(), MempoolError> {
        const RATE_SCRIPT: &str = r#"
            local current = redis.call('INCR', KEYS[1])
            if current == 1 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            if current > tonumber(ARGV[2]) then
                return 1
            end
            return 0
        "#;

        let mut connection = self.connection.lock().await;
        let limited: i64 = redis::Script::new(RATE_SCRIPT)
            .key(self.rate_key(account_id))
            .arg(ttl_secs(window))
            .arg(max_submissions)
            .invoke_async(&mut *connection)
            .await?;
        if limited == 1 {
            return Err(MempoolError::RateLimited { account_id });
        }
        Ok(())
    }

    async fn enqueue_validated(
        &self,
        validated: ValidatedMempoolTx,
        limits: MempoolStoreLimits,
    ) -> Result<(), MempoolError> {
        let payload = serde_json::to_string(&QueuedRedisTx {
            account_id: validated.account_id,
            tx: validated.tx.clone(),
        })?;
        match self
            .try_enqueue_validated(&validated, limits, &payload)
            .await
        {
            Err(MempoolError::GlobalQueueFull) if self.evict_lower_priority(&validated).await? => {
                self.try_enqueue_validated(&validated, limits, &payload)
                    .await
            }
            other => other,
        }
    }

    async fn get_pending(
        &self,
        tx_hash: Hash32,
    ) -> Result<Option<SignedL2Transaction>, MempoolError> {
        let mut connection = self.connection.lock().await;
        let payloads: Vec<String> = redis::cmd("LRANGE")
            .arg(self.queue_key())
            .arg(0)
            .arg(-1)
            .query_async(&mut *connection)
            .await?;
        for payload in payloads {
            let decoded: QueuedRedisTx = serde_json::from_str(&payload)?;
            if decoded.tx.tx_hash() == tx_hash {
                return Ok(Some(decoded.tx));
            }
        }
        Ok(None)
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        let mut connection = self.connection.lock().await;
        let payloads: Vec<String> = redis::cmd("LRANGE")
            .arg(self.queue_key())
            .arg(0)
            .arg(-1)
            .query_async(&mut *connection)
            .await?;
        if payloads.is_empty() || max_txs == 0 {
            return Ok(vec![]);
        }

        let mut queued = Vec::with_capacity(payloads.len());
        for (sequence, payload) in payloads.iter().enumerate() {
            let decoded: QueuedRedisTx = serde_json::from_str(payload)?;
            queued.push(QueuedMempoolTx::from_tx(decoded.tx, sequence as u64));
        }
        let selected_indices = fair_order_indices(&queued, max_txs)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();

        let mut selected = Vec::with_capacity(selected_indices.len());
        let mut retained_payloads = Vec::with_capacity(payloads.len() - selected_indices.len());
        for (index, (queued, payload)) in queued.into_iter().zip(payloads.into_iter()).enumerate() {
            if selected_indices.contains(&index) {
                decrement_account_queue_and_nonce(
                    &mut connection,
                    self.account_queue_key(queued.account_id),
                    self.account_nonce_key(queued.account_id),
                    queued.nonce,
                )
                .await?;
                selected.push(queued.tx);
            } else {
                retained_payloads.push(payload);
            }
        }

        redis::cmd("DEL")
            .arg(self.queue_key())
            .query_async::<()>(&mut *connection)
            .await?;
        if !retained_payloads.is_empty() {
            redis::cmd("RPUSH")
                .arg(self.queue_key())
                .arg(retained_payloads)
                .query_async::<()>(&mut *connection)
                .await?;
        }
        Ok(selected)
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

    async fn stats(&self) -> Result<MempoolStoreStats, MempoolError> {
        let mut connection = self.connection.lock().await;
        let queued_global: usize = redis::cmd("LLEN")
            .arg(self.queue_key())
            .query_async(&mut *connection)
            .await?;
        let leader_locked: bool = redis::cmd("EXISTS")
            .arg(self.leader_key())
            .query_async(&mut *connection)
            .await?;
        let evicted: Option<u64> = redis::cmd("GET")
            .arg(self.evicted_key())
            .query_async(&mut *connection)
            .await?;
        Ok(MempoolStoreStats {
            queued_global,
            queued_accounts: 0,
            evicted: evicted.unwrap_or_default(),
            replay_entries: None,
            nonce_locks: None,
            rate_windows: None,
            leader_locked,
        })
    }
}

impl RedisMempoolStore {
    async fn try_enqueue_validated(
        &self,
        validated: &ValidatedMempoolTx,
        limits: MempoolStoreLimits,
        payload: &str,
    ) -> Result<(), MempoolError> {
        const ENQUEUE_SCRIPT: &str = r#"
            if redis.call('EXISTS', KEYS[1]) == 1 then
                return 1
            end
            if redis.call('EXISTS', KEYS[2]) == 1 then
                return 2
            end
            if redis.call('ZSCORE', KEYS[5], ARGV[6]) then
                return 2
            end
            if redis.call('LLEN', KEYS[3]) >= tonumber(ARGV[3]) then
                return 3
            end
            local account_count = tonumber(redis.call('GET', KEYS[4]) or '0')
            if account_count >= tonumber(ARGV[4]) then
                return 4
            end
            local nonce_count = redis.call('ZCARD', KEYS[5])
            if nonce_count > 0 then
                local min_pair = redis.call('ZRANGE', KEYS[5], 0, 0, 'WITHSCORES')
                local max_pair = redis.call('ZREVRANGE', KEYS[5], 0, 0, 'WITHSCORES')
                local min_nonce = tonumber(min_pair[2])
                local max_nonce = tonumber(max_pair[2])
                local candidate = tonumber(ARGV[6])
                if math.max(max_nonce, candidate) - math.min(min_nonce, candidate) > tonumber(ARGV[5]) then
                    return 5
                end
            end
            redis.call('SET', KEYS[1], '1', 'EX', ARGV[1])
            redis.call('SET', KEYS[2], '1', 'EX', ARGV[2])
            redis.call('INCR', KEYS[4])
            redis.call('EXPIRE', KEYS[4], ARGV[1])
            redis.call('ZADD', KEYS[5], ARGV[6], ARGV[6])
            redis.call('EXPIRE', KEYS[5], ARGV[1])
            redis.call('RPUSH', KEYS[3], ARGV[7])
            return 0
        "#;

        let mut connection = self.connection.lock().await;
        let result: i64 = redis::Script::new(ENQUEUE_SCRIPT)
            .key(self.replay_key(validated.tx_hash))
            .key(self.nonce_key(validated.account_id, validated.nonce))
            .key(self.queue_key())
            .key(self.account_queue_key(validated.account_id))
            .key(self.account_nonce_key(validated.account_id))
            .arg(ttl_secs(limits.replay_ttl))
            .arg(ttl_secs(limits.nonce_lock_ttl))
            .arg(limits.max_global_queue)
            .arg(limits.max_account_queue)
            .arg(limits.max_account_nonce_window)
            .arg(validated.nonce.to_string())
            .arg(payload)
            .invoke_async(&mut *connection)
            .await?;

        match result {
            0 => Ok(()),
            1 => Err(MempoolError::DuplicateTx(validated.tx_hash)),
            2 => Err(MempoolError::NonceLocked {
                account_id: validated.account_id,
                nonce: validated.nonce,
            }),
            3 => Err(MempoolError::GlobalQueueFull),
            4 => Err(MempoolError::AccountQueueFull {
                account_id: validated.account_id,
            }),
            5 => Err(MempoolError::AccountNonceWindowExceeded {
                account_id: validated.account_id,
                nonce: validated.nonce,
                min_nonce: 0,
                max_nonce: 0,
                window: limits.max_account_nonce_window,
            }),
            _ => Err(redis::RedisError::from((
                redis::ErrorKind::ResponseError,
                "unexpected redis enqueue script result",
            ))
            .into()),
        }
    }

    async fn evict_lower_priority(
        &self,
        incoming: &ValidatedMempoolTx,
    ) -> Result<bool, MempoolError> {
        let mut connection = self.connection.lock().await;
        let payloads: Vec<String> = redis::cmd("LRANGE")
            .arg(self.queue_key())
            .arg(0)
            .arg(-1)
            .query_async(&mut *connection)
            .await?;
        let mut worst: Option<(usize, QueuedMempoolTx)> = None;
        for (sequence, payload) in payloads.iter().enumerate() {
            let decoded: QueuedRedisTx = serde_json::from_str(payload)?;
            let queued = QueuedMempoolTx::from_tx(decoded.tx, sequence as u64);
            if worst.as_ref().is_none_or(|(_, current)| {
                queued
                    .priority
                    .cmp(&current.priority)
                    .then_with(|| current.sequence.cmp(&queued.sequence))
                    .then_with(|| queued.tx_hash.cmp(&current.tx_hash))
                    .is_lt()
            }) {
                worst = Some((sequence, queued));
            }
        }
        let Some((worst_index, worst)) = worst else {
            return Ok(false);
        };
        if incoming.priority <= worst.priority {
            return Ok(false);
        }
        let Some(worst_payload) = payloads.get(worst_index) else {
            return Ok(false);
        };
        let removed: i64 = redis::cmd("LREM")
            .arg(self.queue_key())
            .arg(1)
            .arg(worst_payload)
            .query_async(&mut *connection)
            .await?;
        if removed <= 0 {
            return Ok(false);
        }
        decrement_account_queue_and_nonce(
            &mut connection,
            self.account_queue_key(worst.account_id),
            self.account_nonce_key(worst.account_id),
            worst.nonce,
        )
        .await?;
        redis::cmd("INCR")
            .arg(self.evicted_key())
            .query_async::<()>(&mut *connection)
            .await?;
        Ok(true)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct QueuedRedisTx {
    account_id: Hash32,
    tx: SignedL2Transaction,
}

async fn decrement_account_queue_and_nonce(
    connection: &mut ConnectionManager,
    account_queue_key: String,
    account_nonce_key: String,
    nonce: u64,
) -> Result<(), MempoolError> {
    const DECREMENT_SCRIPT: &str = r#"
        local current = redis.call('DECR', KEYS[1])
        if current <= 0 then
            redis.call('DEL', KEYS[1])
        end
        redis.call('ZREM', KEYS[2], ARGV[1])
        if redis.call('ZCARD', KEYS[2]) == 0 then
            redis.call('DEL', KEYS[2])
        end
        return 0
    "#;
    let _: i64 = redis::Script::new(DECREMENT_SCRIPT)
        .key(account_queue_key)
        .key(account_nonce_key)
        .arg(nonce.to_string())
        .invoke_async(connection)
        .await?;
    Ok(())
}

fn ttl_secs(ttl: Duration) -> u64 {
    ttl.as_secs().max(1)
}
