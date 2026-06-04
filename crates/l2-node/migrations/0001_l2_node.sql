CREATE TABLE IF NOT EXISTS l2_blocks (
    height BIGINT PRIMARY KEY CHECK (height >= 0),
    block_hash TEXT NOT NULL UNIQUE,
    prev_block_hash TEXT NOT NULL,
    state_root TEXT NOT NULL,
    data_hash TEXT NOT NULL,
    block_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS l2_transactions (
    tx_hash TEXT PRIMARY KEY,
    block_height BIGINT NOT NULL REFERENCES l2_blocks(height) ON DELETE CASCADE,
    tx_index INTEGER NOT NULL CHECK (tx_index >= 0),
    tx_json JSONB NOT NULL,
    receipt_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (block_height, tx_index)
);

CREATE TABLE IF NOT EXISTS l2_deposits (
    deposit_id TEXT PRIMARY KEY,
    asset_id BIGINT NOT NULL CHECK (asset_id >= 0),
    recipient TEXT NOT NULL,
    amount NUMERIC(39, 0) NOT NULL CHECK (amount > 0),
    l1_tx_hash TEXT NOT NULL,
    l1_lt BIGINT NOT NULL CHECK (l1_lt > 0),
    deposit_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (l1_tx_hash, l1_lt)
);

CREATE TABLE IF NOT EXISTS l2_withdrawals (
    withdrawal_id TEXT PRIMARY KEY,
    block_height BIGINT NOT NULL REFERENCES l2_blocks(height) ON DELETE CASCADE,
    withdrawal_index INTEGER NOT NULL CHECK (withdrawal_index >= 0),
    withdrawal_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (block_height, withdrawal_index)
);

CREATE TABLE IF NOT EXISTS l1_cursors (
    source TEXT PRIMARY KEY,
    lt BIGINT NOT NULL CHECK (lt >= 0),
    hash TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS l2_transactions_block_height_idx
    ON l2_transactions(block_height);

CREATE INDEX IF NOT EXISTS l2_withdrawals_block_height_idx
    ON l2_withdrawals(block_height);

CREATE INDEX IF NOT EXISTS l2_deposits_l1_cursor_idx
    ON l2_deposits(l1_lt, l1_tx_hash);
