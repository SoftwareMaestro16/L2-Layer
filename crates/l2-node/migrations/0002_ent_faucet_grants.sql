CREATE TABLE IF NOT EXISTS ent_faucet_grants (
    account_id TEXT PRIMARY KEY,
    asset_id BIGINT NOT NULL CHECK (asset_id = 0),
    amount NUMERIC(39, 0) NOT NULL CHECK (amount > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
