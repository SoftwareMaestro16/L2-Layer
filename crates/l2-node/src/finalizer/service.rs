use super::{
    BatchFinalizerConfig, FinalizeBatchSignRequest, FinalizeBatchSigner, FinalizerClock,
    FinalizerError, FinalizerSignerOperation, FinalizerStats, OnchainBatchCommitment,
    TonFinalizerProvider, FINALIZATION_LIMIT, SIGN_VALIDITY_SECONDS,
};
use crate::storage::{BatchCommitRecord, BatchCommitStatus, BatchFinalizationStatus, DynStorage};

#[derive(Clone)]
pub struct BatchFinalizer<S, P, C> {
    config: BatchFinalizerConfig,
    storage: DynStorage,
    signer: S,
    provider: P,
    clock: C,
}

impl<S, P, C> BatchFinalizer<S, P, C> {
    pub fn new(
        config: BatchFinalizerConfig,
        storage: DynStorage,
        signer: S,
        provider: P,
        clock: C,
    ) -> Self {
        Self {
            config,
            storage,
            signer,
            provider,
            clock,
        }
    }
}

impl<S, P, C> BatchFinalizer<S, P, C>
where
    S: FinalizeBatchSigner,
    P: TonFinalizerProvider,
    C: FinalizerClock,
{
    pub async fn finalize_once(&self) -> Result<FinalizerStats, FinalizerError> {
        let records = self
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Confirmed],
                u32::MAX,
                FINALIZATION_LIMIT,
            )
            .await?;
        let mut stats = FinalizerStats {
            considered: records.len(),
            ..FinalizerStats::default()
        };
        for record in records {
            match record.finalization_status {
                BatchFinalizationStatus::Submitted => {
                    self.confirm_submitted(record, &mut stats).await?
                }
                BatchFinalizationStatus::Finalized => stats.skipped += 1,
                BatchFinalizationStatus::Pending | BatchFinalizationStatus::Failed => {
                    self.submit_if_eligible(record, &mut stats).await?;
                }
            }
        }
        Ok(stats)
    }

    async fn confirm_submitted(
        &self,
        mut record: BatchCommitRecord,
        stats: &mut FinalizerStats,
    ) -> Result<(), FinalizerError> {
        let Some(message_hash) = record
            .finalize_message_hash_norm
            .or(record.finalize_message_hash)
        else {
            self.mark_failed(&mut record, "finalize message hash missing", true)
                .await?;
            stats.failed += 1;
            return Ok(());
        };
        if !self.provider.message_confirmed(message_hash).await? {
            stats.skipped += 1;
            return Ok(());
        }
        let commitment = self.provider.commitment(record.batch_no).await?;
        self.apply_onchain_commitment(&mut record, &commitment);
        if commitment.finalized {
            self.mark_finalized(record).await?;
            stats.finalized += 1;
        } else {
            self.mark_failed(&mut record, "finalize tx not applied", false)
                .await?;
            stats.failed += 1;
        }
        Ok(())
    }

    async fn submit_if_eligible(
        &self,
        mut record: BatchCommitRecord,
        stats: &mut FinalizerStats,
    ) -> Result<(), FinalizerError> {
        if record.finalization_attempts >= self.config.max_attempts {
            stats.skipped += 1;
            return Ok(());
        }
        let commitment = self.provider.commitment(record.batch_no).await?;
        self.apply_onchain_commitment(&mut record, &commitment);
        if !commitment.exists {
            self.mark_failed(&mut record, "commitment missing", true)
                .await?;
            stats.failed += 1;
            return Ok(());
        }
        if commitment.finalized {
            self.mark_finalized(record).await?;
            stats.finalized += 1;
            return Ok(());
        }
        if self.clock.unix_time() < record.finalization_eligible_at.unwrap_or(u64::MAX) {
            self.storage.save_batch_commit(record).await?;
            stats.not_ready += 1;
            return Ok(());
        }
        self.submit_finalize(record, stats).await
    }

    async fn submit_finalize(
        &self,
        mut record: BatchCommitRecord,
        stats: &mut FinalizerStats,
    ) -> Result<(), FinalizerError> {
        let signed = match self
            .signer
            .sign_finalize_batch(self.sign_request(record.batch_no))
            .await
        {
            Ok(signed) => signed,
            Err(error) => {
                tracing::warn!(?error, batch_no = record.batch_no, "finalize signer failed");
                self.mark_failed(&mut record, "finalize signer failed", true)
                    .await?;
                stats.failed += 1;
                return Ok(());
            }
        };
        if signed.signer_address != self.config.sender_address {
            self.mark_failed(&mut record, "finalize signer address mismatch", true)
                .await?;
            stats.failed += 1;
            return Ok(());
        }
        if signed.boc_base64.trim().is_empty() {
            self.mark_failed(&mut record, "signed finalize boc is empty", true)
                .await?;
            stats.failed += 1;
            return Ok(());
        }
        match self.provider.send_signed_boc(&signed).await {
            Ok(result) => self.mark_submitted(record, result, stats).await?,
            Err(error) => {
                tracing::warn!(?error, batch_no = record.batch_no, "finalize send failed");
                self.mark_failed(&mut record, "ton provider finalize send failed", true)
                    .await?;
                stats.failed += 1;
            }
        }
        Ok(())
    }

    fn sign_request(&self, batch_no: u64) -> FinalizeBatchSignRequest {
        FinalizeBatchSignRequest {
            operation: FinalizerSignerOperation::FinalizeBatch,
            chain_id: self.config.chain_id.clone(),
            rollup_root_address: self.config.rollup_root_address.clone(),
            sender_address: self.config.sender_address.clone(),
            msg_value_nanoton: self.config.finalize_msg_value_nanoton,
            batch_no,
            valid_until: self.clock.unix_time().saturating_add(SIGN_VALIDITY_SECONDS),
        }
    }

    fn apply_onchain_commitment(
        &self,
        record: &mut BatchCommitRecord,
        commitment: &OnchainBatchCommitment,
    ) {
        if let Some(committed_at) = commitment.committed_at {
            record.l1_committed_at = Some(committed_at);
            record.finalization_eligible_at =
                Some(committed_at.saturating_add(self.config.challenge_window_sec));
        }
    }

    async fn mark_submitted(
        &self,
        mut record: BatchCommitRecord,
        result: crate::relayer::TonSubmitResult,
        stats: &mut FinalizerStats,
    ) -> Result<(), FinalizerError> {
        record.finalization_status = BatchFinalizationStatus::Submitted;
        record.finalization_attempts = record.finalization_attempts.saturating_add(1);
        record.finalize_message_hash = Some(result.message_hash);
        record.finalize_message_hash_norm = Some(result.message_hash_norm);
        record.finalization_last_error = None;
        self.storage.save_batch_commit(record).await?;
        stats.submitted += 1;
        Ok(())
    }

    async fn mark_finalized(&self, mut record: BatchCommitRecord) -> Result<(), FinalizerError> {
        record.finalization_status = BatchFinalizationStatus::Finalized;
        record.finalization_last_error = None;
        self.storage.save_batch_commit(record).await?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        record: &mut BatchCommitRecord,
        error: &'static str,
        increment_attempts: bool,
    ) -> Result<(), FinalizerError> {
        record.finalization_status = BatchFinalizationStatus::Failed;
        if increment_attempts {
            record.finalization_attempts = record.finalization_attempts.saturating_add(1);
        }
        record.finalization_last_error = Some(error.to_owned());
        self.storage.save_batch_commit(record.clone()).await?;
        Ok(())
    }
}
