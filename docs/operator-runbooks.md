# Operator Runbooks

This document is for Entropis L2 operators. It covers observability, readiness,
safe diagnostics, and common production/testnet failures.

## Endpoints

Public:

- `GET /healthz`: process is alive. It does not check dependencies.
- `GET /readyz`: dependency readiness for `db`, `redis`, and `ton`.

Admin-only:

- `GET /v1/operator/metrics`: node counters, mempool metrics, relayer/indexer
  counters, and DA/storage latency snapshots.
- `GET /v1/operator/failures`: failed L1 batch relays and current failed
  withdrawal visibility status.
- `GET /v1/operator/batch-relayer`: pending/submitted/failed/latest L1 commit
  relay state.
- `GET /v1/operator/batch-finalizer`: pending/submitted/failed/latest L1
  finalization state.
- `GET /v1/mempool/metrics`: mempool admission and queue metrics.

Admin endpoints require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Do not expose admin endpoints publicly without an authenticated reverse proxy.

## Readiness

`/readyz` returns only safe component codes and latency:

```json
{
  "status": "ready",
  "components": {
    "db": { "ready": true, "code": "ok", "latency_ms": 1 },
    "redis": { "ready": true, "code": "ok", "latency_ms": 1 },
    "ton": { "ready": true, "code": "ok", "latency_ms": 120 }
  }
}
```

Failure codes:

- `db_unavailable`: Postgres health check failed.
- `redis_unavailable`: Redis/mempool store stats failed.
- `ton_unavailable`: Toncenter testnet readiness request failed.

The response must not include `DATABASE_URL`, `REDIS_URL`, `TONCENTER_API_KEY`,
`TONAPI_KEY`, `L2_ADMIN_TOKEN`, or signer credentials.

## Metrics And Alerts

Suggested alert thresholds for testnet:

- Mempool `queued_global` above 80% of `MEMPOOL_MAX_GLOBAL_QUEUE` for 5 minutes.
- Any increase in `node.block_production.errors`.
- No increase in `node.block_production.produced` for 2 block intervals while
  mempool queue is non-empty.
- `node.relayer.failed` increases.
- `node.finalizer.failed` increases.
- `node.indexer.errors` increases for 3 consecutive polls.
- `latency.storage_save_block.max_ms` above 1000 ms.
- `latency.da_write.max_ms` above 1000 ms.
- `/readyz` status is `not_ready`.

The current metrics endpoint is JSON. Prometheus/OpenTelemetry export should map
these counters and latency snapshots without changing the response semantics.

## Common Failures

### Postgres Not Ready

Symptoms:

- `/readyz.components.db.code = "db_unavailable"`.
- Block production logs show `storage error`.

Checks:

- Verify `.env.local` contains the correct `DATABASE_URL`.
- Verify migrations completed on startup.
- Check database connection limits.

Do not paste `DATABASE_URL` into logs or GitHub issues.

### Redis Not Ready

Symptoms:

- `/readyz.components.redis.code = "redis_unavailable"`.
- Public transaction submission fails before sequencer execution.

Checks:

- Verify `.env.local` contains the correct `REDIS_URL`.
- Check Redis connection limits and eviction policy.
- Inspect `/v1/mempool/metrics` when Redis is reachable.

Do not log the Redis password.

### TON Endpoint Not Ready

Symptoms:

- `/readyz.components.ton.code = "ton_unavailable"`.
- Deposit indexer or relayer emits TON provider failures.

Checks:

- Verify `TON_NETWORK=testnet`.
- Verify `TONCENTER_V3_BASE_URL` points to the testnet endpoint.
- Rotate `TONCENTER_API_KEY` if provider rejects requests.
- Back off relayer/indexer polling if the provider is rate-limiting.

Do not expose the API key in readiness responses or trace fields.

### Mempool Backlog

Symptoms:

- `queued_global` grows steadily.
- Rate-limit or payload rejection counters spike.

Actions:

- Check block production is producing blocks.
- Increase `MEMPOOL_POP_BATCH_SIZE` only after checking block gas limits.
- Investigate `rejected` counters for bad signatures or flood attempts.

### Relayer Failures

Symptoms:

- `/v1/operator/failures.relayer_failed_batches` is non-empty.
- `node.relayer.failed` increases.
- `l1_batch_commits.status = failed`.

Actions:

- Check `last_error` for the failed batch.
- If `batch data unavailable`, inspect Postgres mirror storage and, when
  `DA_PUBLIC_BACKEND=filesystem`, the public payload at
  `DA_PUBLIC_FS_DIR/blocks/{height}/{block_hash}-{data_hash}.el2batch` before
  retry.
- If `commit signer failed`, check signer service health and sender address.
- If `commit signer response expired`, check signer clock sync and valid-until
  policy before retry.
- If `signed boc malformed`, check the signer command output and do not broadcast
  the BoC manually.
- If `ton provider send failed`, check Toncenter and retry backoff.

Relayer errors must remain static safe reason strings. Do not store raw provider
responses with secrets in `last_error`.

Signer setup and dry-run signing are documented in
`docs/testnet-signer-service.md`.

### Observer Replay Findings

Symptoms:

- `POST /v1/operator/observer/replay` returns `missing_da`, `corrupt_da`, or
  `invalid`.
- `first_divergence` identifies a batch number, block height, field, or
  transaction index.

Actions:

- For `missing_da`, inspect the public DA reference and Postgres mirror. Do not
  treat this as a state-transition proof; it is an availability finding.
- For `corrupt_da`, compare the payload hash with the commitment `data_hash` and
  restore a known-good public payload before retrying replay.
- For `invalid` with `field=state_root`, `tx_root`, `receipt_root`, or
  `withdrawal_root`, preserve the replay request, DA payload, and checkpoint
  metadata for challenge evidence.
- Do not derive replay commitments from local L2 block JSON for incident review.
  Use RollupRoot readback or an exported commitment list.
- Observer checkpoints are local audit state. They contain replayed L2 state and
  roots, but no wallet secrets or provider API keys.

### Batch Finalizer Failures

Symptoms:

- `/v1/operator/batch-finalizer.failed_finalization` is non-empty.
- `node.finalizer.failed` increases.
- `l1_batch_finalizations.status = failed`.

Actions:

- If `batch commit not confirmed`, check `/v1/operator/batch-relayer` and wait
  for commit confirmation before retry.
- If `finalize signer failed`, check the signer service, bearer token, role, and
  `L2_SIGNER_ROLLUP_ROOT_ADDRESS`.
- If `finalize signer address mismatch`, check `L1_SEQUENCER_SENDER_ADDRESS`
  against the signer public address before retrying.
- If `signed boc malformed`, fix the signer command output and do not broadcast
  the BoC manually.
- If `ton provider finalization send failed`, check Toncenter testnet status and
  retry after the configured backoff.

Finalizer retries are bounded by `L1_BATCH_FINALIZER_MAX_ATTEMPTS`. Persistent
errors must remain static safe reason strings and must not include raw signed
BoCs, provider JSON, signer tokens, or API keys.

### Withdrawal Release Failures

Current node visibility:

- `GET /v1/operator/failures` reports that failed withdrawal indexing is not yet
  enabled.
- On-chain source of truth is `RollupRoot.failedWithdrawal(withdrawalId)` and
  `AssetVault.failedRelease(withdrawalId)`.
- `GET /v1/proof/withdrawal/{withdrawalId}` returns `409` until the containing
  batch is finalized. Treat that as expected waiting state, not a proof failure.

Actions:

- Query the Tolk getters through Acton/Toncenter once contracts are deployed.
- If `RollupRoot.failedWithdrawal(withdrawalId)` exists, call
  `RetryWithdrawal(withdrawalId)` on `RollupRoot`. This retries root-to-vault
  delivery from stored release fields.
- If `AssetVault.failedRelease(withdrawalId)` exists, call
  `RetryRelease(withdrawalId)` on `AssetVault`. This retries vault-to-recipient
  delivery from stored release fields.
- Do not re-submit `ClaimWithdrawal` for a withdrawal that is already marked
  claimed on `RollupRoot`.
- Unsupported asset failures are visible but not retryable until the asset path
  is implemented or registered.
- Never paste a signed BoC, wallet seed, signer token, or provider API key into
  incident notes.

## Incident Response Addendum

These procedures cover L2 surfaces tracked in
`docs/security-audit-l2-roadmap.md`.

### Faucet Abuse Or Backend Failure

Symptoms:

- The faucet queue grows faster than the batch worker drains it.
- Many claims come from the same GitHub user, address, IP, or session.
- Batch grants fail against the node admin faucet endpoint.

Actions:

- Disable the public faucet route or stop the faucet backend before changing
  node state manually.
- Keep `L2_ADMIN_TOKEN` server-side only. If exposure is suspected, rotate it
  and restart the node and faucet backend.
- Restart the faucet backend to clear RAM queue/session/cooldown state when the
  queue is poisoned. This is acceptable for faucet v1 because it is explicitly
  non-durable.
- Enable cooldown enforcement and reduce `FAUCET_MAX_BATCH_SIZE` while
  investigating abuse.
- Preserve safe batch ids, GitHub numeric ids, account ids, timestamps, and
  static error codes. Do not store GitHub OAuth tokens or bearer tokens in
  incident notes.

### Wallet Seed Exposure Report

Symptoms:

- A user reports a leaked seed, browser compromise, or unexpected outgoing
  transaction.
- Wallet UI logs or screenshots appear to contain mnemonic or private key
  material.

Actions:

- Do not ask the user to send a seed, mnemonic, private key, raw signed BoC, or
  wallet export.
- Treat the account as compromised and instruct the user to move remaining
  testnet funds to a fresh account.
- If an operator/admin wallet may be affected, rotate the operator wallet and
  any related admin tokens before resuming demos.
- Preserve only account ids, tx hashes, block heights, and safe static failure
  reasons.
- Do not call the wallet UI production-safe until encrypted IndexedDB/WebCrypto
  storage, lock/unlock, backup confirmation, and transaction review are enabled.

### TVM Emulator Failure Or Nondeterminism

Symptoms:

- Contract calls start returning static TVM adapter errors.
- Replay produces different state roots for the same block and DA payload.
- The configured TVM library is missing, changed, or fails to load.

Actions:

- Stop accepting new public contract deploy/call traffic until deterministic
  replay passes again.
- Preserve block height, tx hash, data hash, code hash, data hash, emulator
  version, and configured library path.
- Re-run observer replay from DA instead of trusting local sequencer storage.
- If needed, switch unsafe public contract support off or back to the
  fail-closed prototype adapter while transfers and bridge flows continue.
- Do not change committed L2 state manually. Fix the adapter or config and
  replay from canonical DA.

### Staking Or Economics Anomaly

Symptoms:

- Fee distribution, reward, commission, or unbonding counters diverge from
  expected deterministic accounting.
- A staking endpoint returns unexpected balances or state transitions.

Actions:

- Keep staking and economics endpoints disabled until the deterministic Rust
  module is implemented and covered by state-machine tests.
- If a future staking feature is enabled, pause new staking actions at the API
  or worker boundary while preserving read-only status endpoints.
- Export relevant block heights, tx hashes, receipts, account ids, and static
  reason codes.
- Do not patch balances manually. Recompute the accounting diff from canonical
  blocks and add a failing regression test before fixing the module.

## Log Safety

Required rules:

- Log hashes, block heights, batch numbers, counters, and safe static reason codes.
- Never log API keys, admin tokens, database URLs, Redis URLs, signer tokens,
  mnemonics, wallet seeds, or raw signed BoCs.
- Avoid logging user-provided strings directly. If an error includes user input,
  map it to a stable reason code before public response or persistent operator
  state.
- Public API internal failures must return safe messages such as `storage error`
  or `data availability error`.

Before every push, run the repository secret scan documented in `AGENTS.md` or an
equivalent staged diff scan.
