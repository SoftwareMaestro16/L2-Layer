# Testnet Batch Finalization E2E

This runbook proves the post-commit finality lane:

```text
confirmed l1_batch_commits row -> RollupRoot.commitment(batchNo)
  -> challenge window elapsed -> signer FinalizeBatch BoC
  -> Toncenter v3 /message -> RollupRoot.commitment(batchNo).finalized
  -> finalization_status = finalized
```

`FinalizeBatch` is permissionless on `RollupRoot`, but the node still uses the
external signer boundary and rejects a returned signer address that does not
match `L1_SEQUENCER_SENDER_ADDRESS`. Local wall-clock time is only a scheduler;
the finalizer reads the on-chain commitment before and after submission.

## Prerequisites

- `docs/testnet-batch-commit-e2e.md` has produced a confirmed batch.
- `L1_BATCH_RELAYER_ENABLED=true`.
- `L1_ROLLUP_ROOT_ADDRESS` points to the verified testnet `RollupRoot`.
- `L1_SEQUENCER_SENDER_ADDRESS` matches the configured signer role.
- `L1_COMMIT_SIGNER_ENDPOINT` accepts typed `finalize_batch` requests.
- `L1_COMMIT_SIGNER_TOKEN` exists only in `.env.local` or process env.
- `L2_CHALLENGE_WINDOW_SEC` matches the public registry and L1 getter.

## Observe Eligibility

Inspect batch state:

```powershell
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/batch-commits
```

Before the challenge window has elapsed, expect:

- `status = "confirmed"`.
- `finalization_status = "pending"`.
- `l1_committed_at` set from `RollupRoot.commitment(batchNo)`.
- `finalization_eligible_at = l1_committed_at + L2_CHALLENGE_WINDOW_SEC`.
- `finalization_attempts = 0`.

The finalizer must not call the signer before `finalization_eligible_at`.

## Sign And Submit FinalizeBatch

After eligibility, the finalizer sends a typed signer request containing:

- `operation = "finalize_batch"`.
- `chain_id`.
- `rollup_root_address`.
- `sender_address`.
- `msg_value_nanoton`.
- `batch_no`.
- `valid_until`.

The node refuses to broadcast when the signer address differs from
`L1_SEQUENCER_SENDER_ADDRESS` or the signed BoC is empty. On successful Toncenter
submission it stores:

- `finalization_status = "submitted"`.
- `finalization_attempts += 1`.
- `finalize_message_hash`.
- `finalize_message_hash_norm`.

## Confirm Finality

The finalizer polls:

```text
GET /api/v3/transactionsByMessage?msg_hash=<finalize_message_hash_norm>&direction=in&limit=1
```

After the message is included, it reads `RollupRoot.commitment(batchNo)`. The row
becomes finalized only when the getter returns `finalized = true`.

Check:

```powershell
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/metrics
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/failures
```

Expected happy path:

- `batch_commits[].finalization_status = "finalized"`.
- `node.finalizer.finalized` increases.
- `node.finalizer.failed` does not increase.
- `finalizer_failed_batches` is empty.

## Safe Failure Reasons

- `commitment missing`: local row is confirmed but root getter has no batch.
- `finalize signer failed`: signer health, auth, allowlist, or token failed.
- `finalize signer address mismatch`: signer role does not match node policy.
- `signed finalize boc is empty`: signer returned an invalid envelope.
- `ton provider finalize send failed`: Toncenter submission failed.
- `finalize message hash missing`: submitted local row lacks a provider hash.
- `finalize tx not applied`: message was included but getter is not finalized.

Retries are bounded by `L1_BATCH_RELAYER_MAX_ATTEMPTS`. Do not reset
`finalization_attempts` until the root cause is fixed.

## Negative Checks

The L1 Tolk suite covers missing, early, and duplicate `FinalizeBatch`
rejections. The Rust finalizer tests cover:

```powershell
cargo test -p l2-node finalizer --lib
```

These tests check challenge-window scheduling, on-chain missing commitment,
signer mismatch before broadcast, submitted confirmation without duplicate send,
and getter parsing for `committedAt` plus `finalized`.
