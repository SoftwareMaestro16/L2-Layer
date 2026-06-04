ALTER TABLE l1_batch_commits
    ADD COLUMN IF NOT EXISTS l1_committed_at BIGINT CHECK (l1_committed_at IS NULL OR l1_committed_at >= 0),
    ADD COLUMN IF NOT EXISTS finalization_eligible_at BIGINT CHECK (finalization_eligible_at IS NULL OR finalization_eligible_at >= 0),
    ADD COLUMN IF NOT EXISTS finalization_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (finalization_status IN ('pending', 'submitted', 'finalized', 'failed')),
    ADD COLUMN IF NOT EXISTS finalization_attempts INTEGER NOT NULL DEFAULT 0 CHECK (finalization_attempts >= 0),
    ADD COLUMN IF NOT EXISTS finalize_message_hash TEXT,
    ADD COLUMN IF NOT EXISTS finalize_message_hash_norm TEXT,
    ADD COLUMN IF NOT EXISTS finalization_last_error TEXT;

CREATE INDEX IF NOT EXISTS l1_batch_commits_finalization_idx
    ON l1_batch_commits (status, finalization_status, finalization_attempts, batch_no);
