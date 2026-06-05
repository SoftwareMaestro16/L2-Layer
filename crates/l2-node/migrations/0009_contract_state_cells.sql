CREATE TABLE IF NOT EXISTS contract_code_cells (
    code_hash TEXT PRIMARY KEY,
    code_boc_base64 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    first_seen_block_height BIGINT NOT NULL REFERENCES l2_blocks(height) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS contract_data_cells (
    data_hash TEXT PRIMARY KEY,
    storage_root TEXT NOT NULL,
    data_boc_base64 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    first_seen_block_height BIGINT NOT NULL REFERENCES l2_blocks(height) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS contract_account_states (
    account_id TEXT PRIMARY KEY,
    code_hash TEXT NOT NULL REFERENCES contract_code_cells(code_hash),
    data_hash TEXT NOT NULL REFERENCES contract_data_cells(data_hash),
    storage_root TEXT NOT NULL,
    last_block_height BIGINT NOT NULL REFERENCES l2_blocks(height) ON DELETE CASCADE,
    account_json JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS contract_account_states_code_hash_idx
    ON contract_account_states(code_hash);

CREATE INDEX IF NOT EXISTS contract_account_states_data_hash_idx
    ON contract_account_states(data_hash);
