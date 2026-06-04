use async_trait::async_trait;
use l2_core::{Hash32, SignedL2Transaction};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::error::MempoolError;
use super::types::{MempoolStore, MempoolStoreLimits, MempoolStoreStats, ValidatedMempoolTx};

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

    fn rate_key(&self, account_id: Hash32) -> String {
        format!("{}:mempool:rate:{}", self.prefix, account_id)
    }

    fn leader_key(&self) -> String {
        format!("{}:mempool:leader", self.prefix)
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
        const ENQUEUE_SCRIPT: &str = r#"
            if redis.call('EXISTS', KEYS[1]) == 1 then
                return 1
            end
            if redis.call('EXISTS', KEYS[2]) == 1 then
                return 2
            end
            if redis.call('LLEN', KEYS[3]) >= tonumber(ARGV[3]) then
                return 3
            end
            local account_count = tonumber(redis.call('GET', KEYS[4]) or '0')
            if account_count >= tonumber(ARGV[4]) then
                return 4
            end
            redis.call('SET', KEYS[1], '1', 'EX', ARGV[1])
            redis.call('SET', KEYS[2], '1', 'EX', ARGV[2])
            redis.call('INCR', KEYS[4])
            redis.call('EXPIRE', KEYS[4], ARGV[1])
            redis.call('RPUSH', KEYS[3], ARGV[5])
            return 0
        "#;

        let payload = serde_json::to_string(&QueuedRedisTx {
            account_id: validated.account_id,
            tx: validated.tx,
        })?;
        let mut connection = self.connection.lock().await;
        let result: i64 = redis::Script::new(ENQUEUE_SCRIPT)
            .key(self.replay_key(validated.tx_hash))
            .key(self.nonce_key(validated.account_id, validated.nonce))
            .key(self.queue_key())
            .key(self.account_queue_key(validated.account_id))
            .arg(ttl_secs(limits.replay_ttl))
            .arg(ttl_secs(limits.nonce_lock_ttl))
            .arg(limits.max_global_queue)
            .arg(limits.max_account_queue)
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
            _ => Err(redis::RedisError::from((
                redis::ErrorKind::ResponseError,
                "unexpected redis enqueue script result",
            ))
            .into()),
        }
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        const POP_SCRIPT: &str = r#"
            local out = {}
            for i = 1, tonumber(ARGV[1]) do
                local payload = redis.call('LPOP', KEYS[1])
                if not payload then
                    break
                end
                table.insert(out, payload)
            end
            return out
        "#;

        let mut connection = self.connection.lock().await;
        let mut out = Vec::with_capacity(max_txs);
        let payloads: Vec<String> = redis::Script::new(POP_SCRIPT)
            .key(self.queue_key())
            .arg(max_txs)
            .invoke_async(&mut *connection)
            .await?;
        for payload in payloads {
            let queued: QueuedRedisTx = serde_json::from_str(&payload)?;
            decrement_account_queue(&mut connection, self.account_queue_key(queued.account_id))
                .await?;
            out.push(queued.tx);
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
        Ok(MempoolStoreStats {
            queued_global,
            queued_accounts: 0,
            replay_entries: None,
            nonce_locks: None,
            rate_windows: None,
            leader_locked,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct QueuedRedisTx {
    account_id: Hash32,
    tx: SignedL2Transaction,
}

async fn decrement_account_queue(
    connection: &mut ConnectionManager,
    key: String,
) -> Result<(), MempoolError> {
    const DECREMENT_SCRIPT: &str = r#"
        local current = redis.call('DECR', KEYS[1])
        if current <= 0 then
            redis.call('DEL', KEYS[1])
        end
        return 0
    "#;
    let _: i64 = redis::Script::new(DECREMENT_SCRIPT)
        .key(key)
        .invoke_async(connection)
        .await?;
    Ok(())
}

fn ttl_secs(ttl: Duration) -> u64 {
    ttl.as_secs().max(1)
}
