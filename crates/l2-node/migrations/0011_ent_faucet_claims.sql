CREATE TABLE IF NOT EXISTS ent_faucet_claims (
    claim_id TEXT PRIMARY KEY CHECK (char_length(claim_id) BETWEEN 1 AND 128),
    batch_id TEXT NOT NULL,
    claim_index INTEGER NOT NULL CHECK (claim_index >= 0),
    account_id TEXT NOT NULL,
    asset_id BIGINT NOT NULL CHECK (asset_id = 0),
    amount NUMERIC(39, 0) NOT NULL CHECK (amount > 0),
    deposit_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('granted', 'duplicate_account', 'failed')
    ),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS ent_faucet_claims_batch_id_idx
    ON ent_faucet_claims(batch_id, claim_index);

CREATE INDEX IF NOT EXISTS ent_faucet_claims_account_id_idx
    ON ent_faucet_claims(account_id);

CREATE INDEX IF NOT EXISTS ent_faucet_claims_created_at_idx
    ON ent_faucet_claims(created_at DESC);
