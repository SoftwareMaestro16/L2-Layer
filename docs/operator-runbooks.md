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
- `GET /v1/mempool/metrics`: mempool admission and queue metrics.

Admin endpoints require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Do not expose admin endpoints publicly without an authenticated reverse proxy.
For live TON testnet startup, use `docs/testnet-runtime-profile.md`; local admin
deposit shortcuts are rejected when `L2_RUNTIME_MODE=testnet-prototype`.

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
- If `batch data unavailable`, inspect DA payload storage before retry.
- If `commit signer failed`, check signer service health and sender address.
- If `ton provider send failed`, check Toncenter and retry backoff.

Relayer errors must remain static safe reason strings. Do not store raw provider
responses with secrets in `last_error`.

### Withdrawal Release Failures

Current node visibility:

- `GET /v1/operator/failures` reports that failed withdrawal indexing is not yet
  enabled.
- On-chain source of truth is `RollupRoot.failedWithdrawal(withdrawalId)` and
  `AssetVault.failedRelease(withdrawalId)`.

Actions:

- Query the Tolk getters through Acton/Toncenter once contracts are deployed.
- Retry through `RetryWithdrawal(withdrawalId)` or `RetryRelease(withdrawalId)`
  when the failed record exists.
- Do not re-submit `ClaimWithdrawal` for a withdrawal that is already marked
  claimed on `RollupRoot`.

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
