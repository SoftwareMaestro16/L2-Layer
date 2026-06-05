use crate::config::NodeConfig;
use crate::relayer::{RelayerError, TonCommitProvider};
use crate::signer::{
    unix_time, FinalizeBatchSignRequest, FinalizeBatchSigner, SignerValidationError,
    DEFAULT_SIGNER_MAX_BODY_BYTES,
};
use crate::storage::{
    BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus, DynStorage,
};

const FINALIZATION_LIMIT: u32 = 20;
const CONFIRMED_COMMIT_SCAN_LIMIT: u32 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchFinalizerConfig {
    pub rollup_root_address: String,
    pub sequencer_sender_address: String,
    pub finalize_msg_value_nanoton: u64,
    pub challenge_window_sec: u32,
    pub poll_interval_ms: u64,
    pub retry_backoff_ms: u64,
    pub max_attempts: u32,
}

impl BatchFinalizerConfig {
    pub fn from_node_config(config: &NodeConfig) -> Option<Self> {
        config.l1_batch_finalizer_enabled.then(|| Self {
            rollup_root_address: config
                .l1_rollup_root_address
                .clone()
                .expect("validated finalizer config has root address"),
            sequencer_sender_address: config
                .l1_sequencer_sender_address
                .clone()
                .expect("validated finalizer config has sender address"),
            finalize_msg_value_nanoton: config.l1_finalize_msg_value_nanoton,
            challenge_window_sec: config.challenge_window_sec,
            poll_interval_ms: config.l1_batch_finalizer_poll_interval_ms,
            retry_backoff_ms: config.l1_batch_finalizer_retry_backoff_ms,
            max_attempts: config.l1_batch_finalizer_max_attempts,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinalizerStats {
    pub created_pending: usize,
    pub considered: usize,
    pub submitted: usize,
    pub finalized: usize,
    pub failed: usize,
    pub waiting: usize,
    pub skipped: usize,
}

#[derive(Clone)]
pub struct BatchFinalizer<S, P> {
    config: BatchFinalizerConfig,
    storage: DynStorage,
    signer: S,
    provider: P,
}

impl<S, P> BatchFinalizer<S, P> {
    pub fn new(config: BatchFinalizerConfig, storage: DynStorage, signer: S, provider: P) -> Self {
        Self {
            config,
            storage,
            signer,
            provider,
        }
    }
}

impl<S, P> BatchFinalizer<S, P>
where
    S: FinalizeBatchSigner,
    P: TonCommitProvider,
{
    pub async fn finalize_once(&self) -> Result<FinalizerStats, RelayerError> {
        let mut stats = FinalizerStats::default();
        stats.finalized += self.confirm_submitted_once().await?;
        stats.created_pending += self.enqueue_confirmed_commits().await?;

        let records = self
            .storage
            .list_batch_finalizations(
                &[
                    BatchFinalizationStatus::Pending,
                    BatchFinalizationStatus::Failed,
                ],
                self.config.max_attempts,
                FINALIZATION_LIMIT,
            )
            .await?;
        stats.considered += records.len();

        for record in records {
            match self.submit_record(record).await? {
                FinalizeOutcome::Submitted => stats.submitted += 1,
                FinalizeOutcome::Failed => stats.failed += 1,
                FinalizeOutcome::Waiting => stats.waiting += 1,
                FinalizeOutcome::Skipped => stats.skipped += 1,
            }
        }
        Ok(stats)
    }

    async fn confirm_submitted_once(&self) -> Result<usize, RelayerError> {
        let records = self
            .storage
            .list_batch_finalizations(
                &[BatchFinalizationStatus::Submitted],
                u32::MAX,
                FINALIZATION_LIMIT,
            )
            .await?;
        let mut finalized = 0;
        for mut record in records {
            let Some(message_hash) = record.message_hash_norm.or(record.message_hash) else {
                continue;
            };
            match self.provider.message_confirmed(message_hash).await {
                Ok(true) => {
                    record.status = BatchFinalizationStatus::Finalized;
                    record.last_error = None;
                    self.storage.save_batch_finalization(record).await?;
                    finalized += 1;
                }
                Ok(false) => {}
                Err(error) => {
                    record.last_error = Some("ton provider finalization confirm failed".to_owned());
                    self.storage.save_batch_finalization(record).await?;
                    return Err(error);
                }
            }
        }
        Ok(finalized)
    }

    async fn enqueue_confirmed_commits(&self) -> Result<usize, RelayerError> {
        let commits = self
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Confirmed],
                u32::MAX,
                CONFIRMED_COMMIT_SCAN_LIMIT,
            )
            .await?;
        let now = unix_time();
        let mut created = 0;
        for commit in commits {
            if self
                .storage
                .get_batch_finalization(commit.batch_no)
                .await?
                .is_some()
            {
                continue;
            }
            let record = BatchFinalizationRecord::pending(
                &commit,
                now.saturating_add(u64::from(self.config.challenge_window_sec)),
            );
            self.storage.save_batch_finalization(record).await?;
            created += 1;
        }
        Ok(created)
    }

    async fn submit_record(
        &self,
        mut record: BatchFinalizationRecord,
    ) -> Result<FinalizeOutcome, RelayerError> {
        if !matches!(
            record.status,
            BatchFinalizationStatus::Pending | BatchFinalizationStatus::Failed
        ) {
            return Ok(FinalizeOutcome::Skipped);
        }
        if record.attempts >= self.config.max_attempts {
            return Ok(FinalizeOutcome::Skipped);
        }
        if unix_time() < record.finalize_after_unix {
            return Ok(FinalizeOutcome::Waiting);
        }

        let Some(commit) = self.storage.get_batch_commit(record.batch_no).await? else {
            self.mark_failed(&mut record, "batch commit missing")
                .await?;
            return Ok(FinalizeOutcome::Failed);
        };
        if commit.status != BatchCommitStatus::Confirmed {
            self.mark_failed(&mut record, "batch commit not confirmed")
                .await?;
            return Ok(FinalizeOutcome::Failed);
        }
        if commit.block_height != record.block_height {
            self.mark_failed(&mut record, "batch finalization commit mismatch")
                .await?;
            return Ok(FinalizeOutcome::Failed);
        }

        let request = FinalizeBatchSignRequest {
            rollup_root_address: self.config.rollup_root_address.clone(),
            sender_address: self.config.sequencer_sender_address.clone(),
            batch_no: record.batch_no,
            msg_value_nanoton: self.config.finalize_msg_value_nanoton,
        };

        let signed = match self.signer.sign_finalize_batch(request).await {
            Ok(signed) => signed,
            Err(error) => {
                tracing::warn!(?error, batch_no = record.batch_no, "finalize signer failed");
                self.mark_failed(&mut record, "finalize signer failed")
                    .await?;
                return Ok(FinalizeOutcome::Failed);
            }
        };
        if signed.signer_address != self.config.sequencer_sender_address {
            self.mark_failed(&mut record, "finalize signer address mismatch")
                .await?;
            return Ok(FinalizeOutcome::Failed);
        }
        if let Err(error) = signed.validate(unix_time(), DEFAULT_SIGNER_MAX_BODY_BYTES) {
            self.mark_failed(&mut record, finalizer_signer_validation_reason(&error))
                .await?;
            return Ok(FinalizeOutcome::Failed);
        }

        let result = match self.provider.send_signed_boc(&signed).await {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    batch_no = record.batch_no,
                    "ton provider finalization send failed"
                );
                self.mark_failed(&mut record, "ton provider finalization send failed")
                    .await?;
                return Ok(FinalizeOutcome::Failed);
            }
        };
        record.status = BatchFinalizationStatus::Submitted;
        record.attempts = record.attempts.saturating_add(1);
        record.message_hash = Some(result.message_hash);
        record.message_hash_norm = Some(result.message_hash_norm);
        record.last_error = None;
        self.storage.save_batch_finalization(record).await?;
        Ok(FinalizeOutcome::Submitted)
    }

    async fn mark_failed(
        &self,
        record: &mut BatchFinalizationRecord,
        error: &'static str,
    ) -> Result<(), RelayerError> {
        record.status = BatchFinalizationStatus::Failed;
        record.attempts = record.attempts.saturating_add(1);
        record.last_error = Some(error.to_owned());
        self.storage.save_batch_finalization(record.clone()).await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FinalizeOutcome {
    Submitted,
    Failed,
    Waiting,
    Skipped,
}

fn finalizer_signer_validation_reason(error: &SignerValidationError) -> &'static str {
    match error {
        SignerValidationError::ExpiredResponse => "finalize signer response expired",
        SignerValidationError::EmptyBoc | SignerValidationError::MalformedBoc => {
            "signed boc malformed"
        }
        SignerValidationError::OversizedBoc => "signed boc oversized",
        SignerValidationError::MissingSignerAddress => "finalize signer address mismatch",
        SignerValidationError::BadRequestId
        | SignerValidationError::ExpiredRequest
        | SignerValidationError::RequestIdMismatch
        | SignerValidationError::ActionMismatch
        | SignerValidationError::InvalidCommitRequest
        | SignerValidationError::InvalidFinalizeRequest => "finalize signer invalid response",
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
