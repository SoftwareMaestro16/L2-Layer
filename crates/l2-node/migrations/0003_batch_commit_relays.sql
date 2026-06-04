CREATE TABLE IF NOT EXISTS l1_batch_commits (
    batch_no BIGINT PRIMARY KEY CHECK (batch_no > 0),
    block_height BIGINT NOT NULL UNIQUE CHECK (block_height >= 0),
    block_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'submitted', 'confirmed', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    message_hash TEXT,
    message_hash_norm TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS l1_batch_commits_status_idx
    ON l1_batch_commits(status, attempts, batch_no);
