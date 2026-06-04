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
- `GET /v1/operator/batch-relayer`
- `GET /v1/operator/batch-finalizer`
- `GET /v1/explorer/summary`
- `GET /v1/explorer/blocks`
- `GET /v1/explorer/deposits`
- `GET /v1/explorer/deposit/{deposit_id_hex}`
- `GET /v1/explorer/withdrawal/{withdrawal_id_hex}`
- `GET /v1/proof/withdrawal/{withdrawal_id_hex}`
- `WS /v1/stream`

`POST /v1/admin/deposit` is a local-development adapter and only works when
`L2_DEV_ADMIN_DEPOSITS_ENABLED=true`. In production/testnet flows, deposits should
come from the TON deposit indexer. Admin endpoints require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Postgres migrations run on startup and create tables for blocks, transactions,
receipts, deposits, withdrawals, L1 cursors, batch DA payloads, L1 batch commit
relays, L1 batch finalizations, and ENT faucet grants.

The ENT faucet is L2-native only in this phase. It grants `ENT_FAUCET_AMOUNT`
whole ENT per account, converted with `ENT_DECIMALS=9`, and requires the admin
bearer token until public rate limiting is implemented.

## Operator observability

`/healthz` is process-alive only. `/readyz` checks Postgres, Redis, and Toncenter
testnet reachability with safe component codes and no secret-bearing config
values. Operator endpoints under `/v1/operator/*` require the admin bearer token
and expose node counters, mempool metrics, relayer/finalizer queues, failures,
and current failed withdrawal visibility.

Use `docs/operator-runbooks.md` for common failure handling, alert thresholds, and
log safety rules.

## Static dashboard

The optional dashboard is a static frontend in `dashboard/`. Open
`dashboard/index.html` and point the API field at the node URL. Public panels use
only public API endpoints. The operator panel asks for the admin bearer token at
runtime, keeps it only in memory, and can read readiness, failure, relayer, and
finalizer views.

Contract links are loaded from a deployment registry URL when one exists, for
example `deployments/testnet/entropis.json`. The dashboard turns RollupRoot and
AssetVault registry addresses into Tonviewer Testnet links.

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

## Jetton bridge testnet flow

Use an existing public testnet Jetton when possible. Deploy a temporary test Jetton
only when no suitable faucet/test asset is available. Do not deploy an ENT L1
Jetton for this prototype path.

Register each bridged Jetton before accepting deposits:

1. Query the Jetton master `get_wallet_address(owner_address)` getter with
   `AssetVault` as owner and verify the resulting vault-owned Jetton wallet.
2. Send `AssetVault.RegisterJettonAsset(assetId, master, wallet, decimals)` from
   the vault admin. Use a non-zero `assetId` that is not `L1_TON_ASSET_ID`.
3. Verify `AssetVault.jettonAsset(assetId)` returns `exists=true`, the expected
   master, the expected vault-owned wallet, and the token decimals.
4. Add the registered id to `L1_DEPOSIT_ASSET_IDS`.

Build user deposits as a TEP-74 Jetton `transfer` to the user's Jetton wallet.
Set `destination` to `AssetVault`, `response_destination` to the user's TON
wallet, `forward_ton_amount > 0`, and the `forward_payload` to the SDK helper's
L2 recipient payload. The SDK emits the canonical ref branch of
`Either Cell ^Cell`; the vault accepts canonical inline or ref branches only when
the decoded payload is exactly one non-zero `uint256` L2 recipient.

```ts
import { depositJettonTonConnectMessage } from "@ton-l2-rollup/sdk";

const message = depositJettonTonConnectMessage({
  jettonWalletAddress: "<user Jetton wallet address>",
  vaultAddress: "<AssetVault testnet address>",
  responseAddress: "<user TON testnet address>",
  queryId: Date.now(),
  jettonAmount: "1000000",
  forwardTonAmount: "50000000",
  tonAmount: "100000000",
  l2Recipient: "<32-byte L2 account id hex>",
});
```

The resulting Jetton wallet notification must come from the registered
vault-owned Jetton wallet, not the master and not the user's wallet. The vault
emits a normal `DepositRecorded` external log, so the existing deposit indexer
credits the configured Jetton `assetId` on L2 after Toncenter v3 polling.

For Jetton withdrawals, the finalized `ReleaseAuthorized` proof path is unchanged:
`RollupRoot` sends `ReleaseAuthorized` to `AssetVault`, and the vault sends
TEP-74 `transfer` to the registered vault-owned Jetton wallet with the claimant
as `destination`. The vault tracks the pending query id, clears it on
`excesses`, and records wallet bounces as retryable `failedRelease` records.

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
request to a local/remote signer service, verifies the returned signer address,
expiry, and BoC shape, then broadcasts the signed external BoC through Toncenter
v3 `/message`. Submitted message hashes are stored in `l1_batch_commits`;
confirmation is checked through Toncenter v3 `/transactionsByMessage`. Retries
are bounded by `L1_BATCH_RELAYER_MAX_ATTEMPTS`.

The signer service is a separate process. Use `docs/testnet-signer-service.md`
for the typed HTTP contract, role split, Acton wallet procedure, and local
no-broadcast dry run.

## TON batch finalizer

The batch finalizer is disabled by default. Enable it after the batch relayer is
confirming commits and the signer can sign `FinalizeBatch`:

```text
L1_BATCH_FINALIZER_ENABLED=true
L1_FINALIZE_SIGNER_ENDPOINT=http://127.0.0.1:8800/sign-finalize
L1_FINALIZE_SIGNER_TOKEN=<local signer bearer token>
L1_FINALIZE_MSG_VALUE_NANOTON=100000000
L1_BATCH_FINALIZER_POLL_INTERVAL_MS=5000
L1_BATCH_FINALIZER_RETRY_BACKOFF_MS=15000
L1_BATCH_FINALIZER_MAX_ATTEMPTS=8
```

When a batch commit becomes confirmed locally, the finalizer creates a
`l1_batch_finalizations` row where `finalize_after_unix` is local confirmation
time plus `L2_CHALLENGE_WINDOW_SEC`. This is conservative: it may wait slightly
longer than the on-chain `committedAt`, but it avoids signing before the
optimistic window. After the delay, the finalizer requests a typed
`FinalizeBatch` BoC, verifies the signer address, expiry, and BoC shape,
broadcasts through Toncenter v3 `/message`, and confirms through
`/transactionsByMessage`.

Operator visibility is available at `GET /v1/operator/batch-finalizer`. The
response groups `pending_finalization`, `submitted_finalization`,
`failed_finalization`, `latest`, and `latest_finalized`. Persistent error fields
use static safe reason codes only.

## Withdrawal operations

After a finalized batch, users claim withdrawals through `RollupRoot.ClaimWithdrawal`.
The public proof endpoint is intentionally finality-gated: before the related
batch is finalized, `GET /v1/proof/withdrawal/{withdrawal_id_hex}` returns HTTP
`409` with `withdrawal batch not finalized`.

End-to-end testnet claim flow:

1. Build and sign an L2 withdrawal transaction with the SDK helper:

   ```ts
   import { buildWithdrawTransaction, signTransaction } from "@ton-l2-rollup/sdk";

   const unsigned = buildWithdrawTransaction({
     chainId: "entropis-testnet",
     from: "<l2 account id hex>",
     nonce: 0,
     assetId: 1,
     amount: "100000000",
     l1Recipient: "<recipient TON testnet address>",
     gasLimit: 1000,
     maxGasPrice: "1",
   });
   const tx = signTransaction(unsigned, keyPair);
   ```

2. Submit the transaction to `POST /v1/tx`.
3. Wait for sequencer inclusion, batch relay confirmation, and batch
   finalization. Operator visibility is available at
   `GET /v1/operator/batch-relayer` and `GET /v1/operator/batch-finalizer`.
4. Fetch the finalized withdrawal proof:

   ```text
   GET /v1/proof/withdrawal/<withdrawal_id_hex>
   ```

5. Build a `ClaimWithdrawal` body for a TON wallet, signer, or TON Connect flow:

   ```ts
   import { claimWithdrawalTonConnectMessage } from "@ton-l2-rollup/sdk";

   const message = claimWithdrawalTonConnectMessage({
     rollupRootAddress: "<RollupRoot testnet address>",
     proof,
     amount: "150000000",
   });
   ```

6. Send the raw internal message body to `RollupRoot`. The root verifies the
   `ReleaseAuthorized` leaf cell and compact Merkle proof, marks the withdrawal
   claimed, and asks `AssetVault` to release TON to the recipient.

If a root-to-vault release bounces, operators or users can inspect
`RollupRoot.failedWithdrawal(withdrawalId)` and retry with
`RollupRoot.RetryWithdrawal(withdrawalId)`.

If a vault-to-recipient TON release bounces, `AssetVault` records
`failedRelease(withdrawalId)` and re-credits TON asset custody accounting. Retry is
permissionless through `AssetVault.RetryRelease(withdrawalId)` and uses the stored
failure fields only. Registered Jetton withdrawals retry through the vault-owned
Jetton wallet. Unsupported asset ids remain failed and are not retryable until the
asset is registered or a future wrapped-gas flow is implemented.

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

## L1 testnet deployment

`RollupRoot` and `AssetVault` deploy through Acton scripts under `scripts/l1`.
The local emulation path is:

```powershell
wsl acton run l1-deploy-plan -- <sequencer-address> <wrapped-gas-minter-address> 300 1 9
```

The testnet path is:

```powershell
wsl acton run l1-deploy-testnet -- <sequencer-address> <wrapped-gas-minter-address> 300 1 9
```

The deployment script writes ignored JSON to `L1_DEPLOY_OUTPUT_JSON`, defaulting to
`build/testnet-l1-deployment.json`. Use `docs/testnet-l1-deployment.md` for wallet
setup, replay safety, and getter verification. Do not run deployment scripts with
`--net mainnet`.
