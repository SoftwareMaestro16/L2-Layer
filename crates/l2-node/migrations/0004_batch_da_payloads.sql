CREATE TABLE IF NOT EXISTS l2_batch_payloads (
    block_height BIGINT PRIMARY KEY CHECK (block_height >= 0),
    block_hash TEXT NOT NULL,
    data_hash TEXT NOT NULL,
    payload_bytes BYTEA NOT NULL,
    payload_size BIGINT NOT NULL CHECK (payload_size >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (block_hash, data_hash)
);

CREATE INDEX IF NOT EXISTS l2_batch_payloads_data_hash_idx
    ON l2_batch_payloads(data_hash);
