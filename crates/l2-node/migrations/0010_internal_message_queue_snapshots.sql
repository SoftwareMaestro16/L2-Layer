CREATE TABLE IF NOT EXISTS internal_message_queue_snapshots (
    block_height BIGINT PRIMARY KEY REFERENCES l2_blocks(height) ON DELETE CASCADE,
    queue_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
