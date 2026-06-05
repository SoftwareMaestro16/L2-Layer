CREATE TABLE IF NOT EXISTS contract_source_submissions (
    submission_id TEXT PRIMARY KEY,
    code_hash TEXT NOT NULL,
    account_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('pending', 'verified', 'rejected')),
    files_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS contract_source_submissions_code_hash_idx
    ON contract_source_submissions(code_hash);

CREATE INDEX IF NOT EXISTS contract_source_submissions_status_idx
    ON contract_source_submissions(status);
