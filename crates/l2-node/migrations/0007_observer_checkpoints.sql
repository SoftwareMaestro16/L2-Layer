CREATE TABLE IF NOT EXISTS observer_checkpoints (
    next_batch_no BIGINT PRIMARY KEY,
    next_block_height BIGINT NOT NULL,
    state_root TEXT NOT NULL,
    checkpoint_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
