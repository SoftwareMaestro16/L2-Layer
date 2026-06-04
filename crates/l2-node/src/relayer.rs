use crate::config::{NodeConfig, SecretString};
use crate::storage::{BatchCommitRecord, BatchCommitStatus, DynStorage, StorageError};
use async_trait::async_trait;
use base64::prelude::{Engine as _, BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use l2_core::{Hash32, L2Block, L2BlockHeader};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const SUBMISSION_LIMIT: u32 = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRelayerConfig {
    pub rollup_root_address: String,
    pub sequencer_sender_address: String,
    pub commit_msg_value_nanoton: u64,
    pub poll_interval_ms: u64,
    pub retry_backoff_ms: u64,
    pub max_attempts: u32,
}

impl BatchRelayerConfig {
    pub fn from_node_config(config: &NodeConfig) -> Option<Self> {
        config.l1_batch_relayer_enabled.then(|| Self {
            rollup_root_address: config
                .l1_rollup_root_address
                .clone()
                .expect("validated relayer config has root address"),
            sequencer_sender_address: config
                .l1_sequencer_sender_address
                .clone()
                .expect("validated relayer config has sender address"),
            commit_msg_value_nanoton: config.l1_commit_msg_value_nanoton,
            poll_interval_ms: config.l1_batch_relayer_poll_interval_ms,
            retry_backoff_ms: config.l1_batch_relayer_retry_backoff_ms,
            max_attempts: config.l1_batch_relayer_max_attempts,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchRootsA {
    pub prev_state_root: Hash32,
    pub state_root: Hash32,
    pub tx_root: Hash32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchRootsB {
    pub receipt_root: Hash32,
    pub withdrawal_root: Hash32,
    pub data_hash: Hash32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchCommitment {
    pub batch_no: u64,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub roots_a: BatchRootsA,
    pub roots_b: BatchRootsB,
}

impl BatchCommitment {
    pub fn from_block(block: &L2Block) -> Result<Self, RelayerError> {
        Self::from_header(&block.header)
    }

    pub fn from_header(header: &L2BlockHeader) -> Result<Self, RelayerError> {
        let batch_no = header
            .height
            .checked_add(1)
            .ok_or(RelayerError::Validation(
                "block height cannot be converted to L1 batch number",
            ))?;
        Ok(Self {
            batch_no,
            block_height: header.height,
            block_hash: header.block_hash(),
            roots_a: BatchRootsA {
                prev_state_root: header.prev_state_root,
                state_root: header.state_root,
                tx_root: header.tx_root,
            },
            roots_b: BatchRootsB {
                receipt_root: header.receipt_root,
                withdrawal_root: header.withdrawal_root,
                data_hash: header.data_hash,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitBatchSignRequest {
    pub rollup_root_address: String,
    pub sender_address: String,
    pub msg_value_nanoton: u64,
    pub commitment: BatchCommitment,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedCommitBatch {
    pub boc_base64: String,
    pub signer_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TonSubmitResult {
    pub message_hash: Hash32,
    pub message_hash_norm: Hash32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelayerStats {
    pub considered: usize,
    pub submitted: usize,
    pub confirmed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[async_trait]
pub trait CommitBatchSigner: Send + Sync {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, RelayerError>;
}

#[async_trait]
pub trait TonCommitProvider: Send + Sync {
    async fn send_signed_boc(
        &self,
        signed: &SignedCommitBatch,
    ) -> Result<TonSubmitResult, RelayerError>;

    async fn message_confirmed(&self, message_hash: Hash32) -> Result<bool, RelayerError>;
}

#[derive(Clone, Debug)]
pub struct RemoteCommitBatchSigner {
    endpoint: String,
    token: SecretString,
    client: reqwest::Client,
}

impl RemoteCommitBatchSigner {
    pub fn from_config(config: &NodeConfig) -> Option<Self> {
        Some(Self {
            endpoint: config.l1_commit_signer_endpoint.clone()?,
            token: config.l1_commit_signer_token.clone()?,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl CommitBatchSigner for RemoteCommitBatchSigner {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, RelayerError> {
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.token.expose())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<RemoteSignerResponse>()
            .await?;
        Ok(SignedCommitBatch {
            boc_base64: response.boc_base64,
            signer_address: response.signer_address,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RemoteSignerResponse {
    boc_base64: String,
    signer_address: String,
}

#[derive(Clone, Debug)]
pub struct ToncenterCommitProvider {
    base_url: String,
    api_key: SecretString,
    client: reqwest::Client,
}

impl ToncenterCommitProvider {
    pub fn from_config(config: &NodeConfig) -> Self {
        Self {
            base_url: config
                .toncenter_v3_base_url
                .trim_end_matches('/')
                .to_owned(),
            api_key: config.toncenter_api_key.clone(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TonCommitProvider for ToncenterCommitProvider {
    async fn send_signed_boc(
        &self,
        signed: &SignedCommitBatch,
    ) -> Result<TonSubmitResult, RelayerError> {
        let response = self
            .client
            .post(format!("{}/message", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .json(&serde_json::json!({ "boc": signed.boc_base64 }))
            .send()
            .await?
            .error_for_status()?
            .json::<ToncenterSendMessageResponse>()
            .await?;
        Ok(TonSubmitResult {
            message_hash: parse_hash_or_base64(&response.message_hash)?,
            message_hash_norm: parse_hash_or_base64(&response.message_hash_norm)?,
        })
    }

    async fn message_confirmed(&self, message_hash: Hash32) -> Result<bool, RelayerError> {
        let response = self
            .client
            .get(format!("{}/transactionsByMessage", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .query(&[
                ("msg_hash", message_hash.to_hex()),
                ("direction", "in".to_owned()),
                ("limit", "1".to_owned()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<ToncenterTransactionsByMessageResponse>()
            .await?;
        Ok(!response.transactions.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct ToncenterSendMessageResponse {
    message_hash: String,
    message_hash_norm: String,
}

#[derive(Debug, Deserialize)]
struct ToncenterTransactionsByMessageResponse {
    #[serde(default)]
    transactions: Vec<Value>,
}

#[derive(Clone)]
pub struct BatchRelayer<S, P> {
    config: BatchRelayerConfig,
    storage: DynStorage,
    signer: S,
    provider: P,
}

impl<S, P> BatchRelayer<S, P> {
    pub fn new(config: BatchRelayerConfig, storage: DynStorage, signer: S, provider: P) -> Self {
        Self {
            config,
            storage,
            signer,
            provider,
        }
    }
}

impl<S, P> BatchRelayer<S, P>
where
    S: CommitBatchSigner,
    P: TonCommitProvider,
{
    pub async fn relay_once(&self) -> Result<RelayerStats, RelayerError> {
        let mut stats = RelayerStats::default();
        stats.confirmed += self.confirm_submitted_once().await?;

        let records = self
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Pending, BatchCommitStatus::Failed],
                self.config.max_attempts,
                SUBMISSION_LIMIT,
            )
            .await?;
        stats.considered += records.len();

        for record in records {
            match self.submit_record(record).await? {
                SubmitOutcome::Submitted => stats.submitted += 1,
                SubmitOutcome::Failed => stats.failed += 1,
                SubmitOutcome::Skipped => stats.skipped += 1,
            }
        }
        Ok(stats)
    }

    async fn confirm_submitted_once(&self) -> Result<usize, RelayerError> {
        let records = self
            .storage
            .list_batch_commits(&[BatchCommitStatus::Submitted], u32::MAX, SUBMISSION_LIMIT)
            .await?;
        let mut confirmed = 0;
        for mut record in records {
            let Some(message_hash) = record.message_hash_norm.or(record.message_hash) else {
                continue;
            };
            if self.provider.message_confirmed(message_hash).await? {
                record.status = BatchCommitStatus::Confirmed;
                record.last_error = None;
                self.storage.save_batch_commit(record).await?;
                confirmed += 1;
            }
        }
        Ok(confirmed)
    }

    async fn submit_record(
        &self,
        mut record: BatchCommitRecord,
    ) -> Result<SubmitOutcome, RelayerError> {
        if !matches!(
            record.status,
            BatchCommitStatus::Pending | BatchCommitStatus::Failed
        ) {
            return Ok(SubmitOutcome::Skipped);
        }
        if record.attempts >= self.config.max_attempts {
            return Ok(SubmitOutcome::Skipped);
        }

        let block = match self.storage.get_block(record.block_height).await? {
            Some(block) => block,
            None => {
                self.mark_failed(&mut record, "l2 block missing").await?;
                return Ok(SubmitOutcome::Failed);
            }
        };
        if block.header.block_hash() != record.block_hash {
            self.mark_failed(&mut record, "l2 block hash mismatch")
                .await?;
            return Ok(SubmitOutcome::Failed);
        }

        let request = CommitBatchSignRequest {
            rollup_root_address: self.config.rollup_root_address.clone(),
            sender_address: self.config.sequencer_sender_address.clone(),
            msg_value_nanoton: self.config.commit_msg_value_nanoton,
            commitment: BatchCommitment::from_block(&block)?,
        };

        let signed = match self.signer.sign_commit_batch(request).await {
            Ok(signed) => signed,
            Err(error) => {
                tracing::warn!(?error, batch_no = record.batch_no, "commit signer failed");
                self.mark_failed(&mut record, "commit signer failed")
                    .await?;
                return Ok(SubmitOutcome::Failed);
            }
        };
        if signed.signer_address != self.config.sequencer_sender_address {
            self.mark_failed(&mut record, "commit signer address mismatch")
                .await?;
            return Ok(SubmitOutcome::Failed);
        }
        if signed.boc_base64.trim().is_empty() {
            self.mark_failed(&mut record, "signed boc is empty").await?;
            return Ok(SubmitOutcome::Failed);
        }

        let result = match self.provider.send_signed_boc(&signed).await {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    batch_no = record.batch_no,
                    "ton provider send failed"
                );
                self.mark_failed(&mut record, "ton provider send failed")
                    .await?;
                return Ok(SubmitOutcome::Failed);
            }
        };
        record.status = BatchCommitStatus::Submitted;
        record.attempts = record.attempts.saturating_add(1);
        record.message_hash = Some(result.message_hash);
        record.message_hash_norm = Some(result.message_hash_norm);
        record.last_error = None;
        self.storage.save_batch_commit(record).await?;
        Ok(SubmitOutcome::Submitted)
    }

    async fn mark_failed(
        &self,
        record: &mut BatchCommitRecord,
        error: &'static str,
    ) -> Result<(), RelayerError> {
        record.status = BatchCommitStatus::Failed;
        record.attempts = record.attempts.saturating_add(1);
        record.last_error = Some(error.to_owned());
        self.storage.save_batch_commit(record.clone()).await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubmitOutcome {
    Submitted,
    Failed,
    Skipped,
}

#[derive(Debug, Error)]
pub enum RelayerError {
    #[error("relayer validation failed: {0}")]
    Validation(&'static str),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("ton relayer HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("ton relayer decoding failed: {0}")]
    Decode(&'static str),
    #[error("commit signer failed: {0}")]
    Signer(String),
    #[error("ton provider failed: {0}")]
    Provider(String),
}

fn parse_hash_or_base64(value: &str) -> Result<Hash32, RelayerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| RelayerError::Decode("bad hash hex"));
    }

    let decoded = BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| RelayerError::Decode("bad hash encoding"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| RelayerError::Decode("hash must be 32 bytes"))?;
    Ok(Hash32::new(bytes))
}

#[cfg(test)]
#[path = "relayer_tests.rs"]
mod tests;
