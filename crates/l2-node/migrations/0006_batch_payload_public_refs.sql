ALTER TABLE l2_batch_payloads
    ADD COLUMN IF NOT EXISTS public_ref TEXT,
    ADD COLUMN IF NOT EXISTS public_uri TEXT;

CREATE INDEX IF NOT EXISTS l2_batch_payloads_public_ref_idx
    ON l2_batch_payloads(public_ref)
    WHERE public_ref IS NOT NULL;
