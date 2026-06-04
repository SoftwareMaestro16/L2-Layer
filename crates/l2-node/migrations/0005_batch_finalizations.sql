CREATE TABLE IF NOT EXISTS l1_batch_finalizations (
    batch_no BIGINT PRIMARY KEY REFERENCES l1_batch_commits(batch_no) ON DELETE CASCADE,
    block_height BIGINT NOT NULL CHECK (block_height >= 0),
    status TEXT NOT NULL CHECK (status IN ('pending', 'submitted', 'finalized', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    finalize_after_unix BIGINT NOT NULL CHECK (finalize_after_unix >= 0),
    message_hash TEXT,
    message_hash_norm TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS l1_batch_finalizations_status_idx
    ON l1_batch_finalizations(status, attempts, batch_no);

CREATE INDEX IF NOT EXISTS l1_batch_finalizations_finalize_after_idx
    ON l1_batch_finalizations(finalize_after_unix, status);
