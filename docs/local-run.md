# Local Run

## Rust node

Create a local secrets file from the tracked template:

```powershell
Copy-Item .env.example .env.local
```

Fill `.env.local` with testnet-only values. The file is ignored by git. Required
runtime keys:

- `TON_NETWORK=testnet`
- `TONCENTER_V3_BASE_URL=https://testnet.toncenter.com/api/v3`
- `TONCENTER_API_KEY`
- `TONAPI_BASE_URL=https://testnet.tonapi.io`
- `TONAPI_KEY`
- `DATABASE_URL`
- `REDIS_URL`
- `L2_ADMIN_TOKEN`
- `ENT_DECIMALS=9`
- `ENT_LOGO_PATH=assets/entropis.png`
- `ENT_FAUCET_REQUIRE_ADMIN=true`
- `L2_DEV_ADMIN_DEPOSITS_ENABLED=true` for local-only manual deposits
- `L1_DEPOSIT_INDEXER_ENABLED=false` until a testnet `AssetVault` address is deployed
- `L1_BATCH_RELAYER_ENABLED=false` until `RollupRoot` and a sequencer signer are ready

`l2-node` refuses mainnet config and redacts secret values from debug logs.

```powershell
cargo run -p l2-node
```

Useful endpoints:

- `POST /v1/tx`
- `POST /v1/admin/deposit`
- `POST /v1/admin/faucet/ent`
- `POST /v1/admin/produce-block`
- `GET /v1/account/{account_id_hex}`
- `GET /v1/block/{height}`
- `GET /v1/tx/{tx_hash_hex}`
- `GET /readyz`
- `GET /v1/mempool/metrics`
- `GET /v1/operator/metrics`
- `GET /v1/operator/failures`
- `GET /v1/proof/withdrawal/{withdrawal_id_hex}`
- `WS /v1/stream`

`POST /v1/admin/deposit` is a local-development adapter and only works when
`L2_DEV_ADMIN_DEPOSITS_ENABLED=true`. In production/testnet flows, deposits should
come from the TON deposit indexer. Admin endpoints require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Postgres migrations run on startup and create tables for blocks, transactions,
receipts, deposits, withdrawals, L1 cursors, batch DA payloads, and ENT faucet
grants.

The ENT faucet is L2-native only in this phase. It grants `ENT_FAUCET_AMOUNT`
whole ENT per account, converted with `ENT_DECIMALS=9`, and requires the admin
bearer token until public rate limiting is implemented.

## Operator observability

`/healthz` is process-alive only. `/readyz` checks Postgres, Redis, and Toncenter
testnet reachability with safe component codes and no secret-bearing config
values. Operator endpoints under `/v1/operator/*` require the admin bearer token
and expose node counters, mempool metrics, relayer failures, and current failed
withdrawal visibility.

Use `docs/operator-runbooks.md` for common failure handling, alert thresholds, and
log safety rules.

## Mempool admission limits

Public `POST /v1/tx` requests are admitted through a fail-closed mempool policy
before they reach the sequencer. The defaults are conservative for testnet and
can be tuned through environment variables:

```text
MEMPOOL_REPLAY_TTL_SECS=86400
MEMPOOL_NONCE_LOCK_TTL_SECS=300
MEMPOOL_LEADER_TTL_SECS=10
MEMPOOL_RATE_LIMIT_WINDOW_SECS=60
MEMPOOL_MAX_GLOBAL_QUEUE=10000
MEMPOOL_MAX_ACCOUNT_QUEUE=64
MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW=120
MEMPOOL_MAX_PAYLOAD_BYTES=16384
MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES=8192
MEMPOOL_MIN_GAS_LIMIT=1
MEMPOOL_MAX_GAS_LIMIT=1000000
MEMPOOL_MIN_GAS_PRICE=1
MEMPOOL_MAX_TX_FEE=1000000000000
MEMPOOL_POP_BATCH_SIZE=1024
```

The mempool rejects duplicate transaction hashes, locked account nonces, malformed
signatures, wrong chain ids, zero/oversized gas policies, oversized public payloads,
oversized or malformed `CallContract.body_boc_base64`, per-account queue floods,
global queue floods, and per-account rate-limit abuse. Bad-signature submissions
with a valid sender/public-key pair consume the same per-account rate limit as
valid submissions. `GET /v1/mempool/metrics` exposes accepted/rejected counters
and current store queue depth for operators.

## Executor gas schedule

The executor uses a versioned gas schedule for consensus-critical fee debits and
receipt roots:

```text
EXECUTOR_GAS_SCHEDULE_VERSION=1
EXECUTOR_TRANSFER_GAS=10
EXECUTOR_WITHDRAW_GAS=20
EXECUTOR_CALL_CONTRACT_GAS=50
EXECUTOR_REJECTED_EXECUTION_GAS=1
EXECUTOR_MIN_GAS_PRICE=1
```

For user transactions, the charged fee is `gas_used * max_gas_price` in the
configured gas coin asset, currently ENT asset id `0`. Transfers and withdrawals
debit the moved asset and gas coin separately unless the moved asset is also the
gas coin; in that case `amount + fee` is checked with overflow-safe arithmetic.

Rejected execution uses no-refund MVP semantics: if the transaction passed
sequencer auth/nonce checks and reached the executor, the sender nonce advances
and the executor attempts to charge `EXECUTOR_REJECTED_EXECUTION_GAS *
max_gas_price`. Sequencer-level rejections such as bad signatures, wrong chain id,
or bad nonce are not charged because they are rejected before execution.

`CallContract` requires `body_boc_base64` to decode into a valid single-root TON
BoC. Valid calls currently reach the noop TVM adapter and are rejected with
`tvm_adapter_not_implemented`; malformed BoCs are rejected earlier with
`malformed_boc`. The real adapter must run locally or in an isolated deterministic
worker boundary and must not call external networks from the sequencer path.

## Data availability

The MVP stores canonical batch payload bytes in Postgres before saving a block as
pending for L1 relay:

```text
DA_MAX_PAYLOAD_BYTES=8388608
```

The payload is the consensus `BatchData` bytes, not JSON. `data_hash` in the L2
block header is derived from those bytes. Before a batch is submitted to
`RollupRoot`, the relayer reads the payload back and rejects missing, corrupted,
partial, oversized, or wrong-block payloads without calling the signer or TON
provider. Future TON Storage support should implement the same `DaWriter`,
`DaReader`, and `DaVerifier` boundaries.

## TON deposit indexer

The deposit indexer is disabled by default. Enable it only after `AssetVault` is
deployed to TON testnet:

```text
L1_DEPOSIT_INDEXER_ENABLED=true
L1_VAULT_ADDRESS=<vault address as returned by Toncenter v3>
L1_DEPOSIT_POLL_INTERVAL_MS=5000
L1_DEPOSIT_BATCH_LIMIT=100
L1_DEPOSIT_CONFIRMATION_LAG_LT=0
L1_TON_ASSET_ID=1
L1_DEPOSIT_ASSET_IDS=1,2
```

It polls Toncenter v3 `/messages` for `DepositRecorded` external logs emitted by
the configured vault, stores progress in `l1_cursors`, saves deposits idempotently,
and feeds new deposits into the sequencer. `L1_DEPOSIT_ASSET_IDS` is the whitelist
of vault-registered L1 assets accepted by the indexer; keep `1` for bridged TON and
add registered Jetton asset ids after `RegisterJettonAsset`. Malformed expected logs
fail closed and do not advance the cursor.

## TON batch relayer

The batch relayer is disabled by default. Enable it only after `RollupRoot` is
deployed and its `sequencer` storage address matches the configured sender:

```text
L1_BATCH_RELAYER_ENABLED=true
L1_ROLLUP_ROOT_ADDRESS=<rollup root address>
L1_SEQUENCER_SENDER_ADDRESS=<wallet address authorized as RollupRoot.sequencer>
L1_COMMIT_SIGNER_ENDPOINT=http://127.0.0.1:8800/sign-commit
L1_COMMIT_SIGNER_TOKEN=<local signer bearer token>
L1_COMMIT_MSG_VALUE_NANOTON=100000000
L1_BATCH_RELAYER_POLL_INTERVAL_MS=5000
L1_BATCH_RELAYER_RETRY_BACKOFF_MS=15000
L1_BATCH_RELAYER_MAX_ATTEMPTS=8
```

The node does not store raw wallet credentials. It sends a `CommitBatch` signing
request to a local/remote signer service, verifies the returned signer address
matches `L1_SEQUENCER_SENDER_ADDRESS`, then broadcasts the signed external BoC
through Toncenter v3 `/message`. Submitted message hashes are stored in
`l1_batch_commits`; confirmation is checked through Toncenter v3
`/transactionsByMessage`. Retries are bounded by `L1_BATCH_RELAYER_MAX_ATTEMPTS`.

## Withdrawal operations

After a finalized batch, users claim withdrawals through `RollupRoot.ClaimWithdrawal`.
If a root-to-vault release bounces, operators or users can inspect
`RollupRoot.failedWithdrawal(withdrawalId)` and retry with
`RollupRoot.RetryWithdrawal(withdrawalId)`.

If a vault-to-recipient release bounces, `AssetVault` records
`failedRelease(withdrawalId)` and re-credits TON asset custody accounting. Retry is
permissionless through `AssetVault.RetryRelease(withdrawalId)` and uses the stored
failure fields only. Unsupported asset ids remain failed and are not retryable
until the Jetton/wrapped-gas release path is implemented.

## Acton

Acton must be installed before Tolk contracts can be built and wrappers generated.
The project pins Acton `1.1.0` in `Acton.toml`. Native Windows Acton is not part
of the supported local path; run checks in Linux, WSL, or the pinned Docker image.

Linux or WSL setup:

```bash
curl -LsSf https://github.com/ton-blockchain/acton/releases/latest/download/acton-installer.sh | sh
exec "$SHELL" -l
acton up 1.1.0
bash scripts/ci/acton_contract_checks.sh
```

The shared check script runs:

```text
acton --version
acton doctor
acton build
acton test
acton check
acton fmt --check
```

From PowerShell, use WSL:

```powershell
wsl bash scripts/ci/acton_contract_checks.sh
```

If Acton is unavailable in WSL but Docker is available, use the pinned fallback:

```powershell
wsl env ACTON_USE_DOCKER=1 bash scripts/ci/acton_contract_checks.sh
```

The fallback image is `ghcr.io/ton-blockchain/acton:1.1.0`. It mounts the
repository at `/workspace`, runs with `HOME=/tmp/acton-home` and
`XDG_CACHE_HOME=/tmp/acton-cache`, and does not mount host wallet directories.
Only safe CI flags are passed through by the script; deployment secrets stay in
`.env.local` or the operator environment and are not needed for contract checks.

Wrapper generation is separate from validation and should be committed only when
contract ABI changes require regenerated wrappers:

```powershell
wsl acton wrapper RollupRoot --ts
wsl acton wrapper AssetVault --ts
```

Acton local validation must not use `--net mainnet`. Deployment and verification
scripts should use explicit testnet runbooks once testnet addresses and signer
boundaries are ready.
