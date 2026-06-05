# Local Run

For the full public TON testnet launch sequence, use
`docs/testnet-launch-runbook.md`. This file is the lower-level local operator
reference.

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
- `GET /v1/block/{height}/finality`
- `GET /v1/tx/{tx_hash_hex}`
- `GET /v1/tx/{tx_hash_hex}/receipt`
- `GET /v1/receipt/{tx_hash_hex}`
- `GET /v1/contract/{contract_id}/state`
- `POST /v1/contract/{contract_id}/get-method`
- `GET /v1/da/batch/{height}`
- `GET /v1/da/batch/{height}/{data_hash_hex}`
- `GET /readyz`
- `GET /v1/mempool/metrics`
- `GET /v1/operator/metrics`
- `GET /v1/operator/failures`
- `GET /v1/operator/batch-relayer`
- `GET /v1/operator/batch-finalizer`
- `GET /v1/operator/observer/checkpoint`
- `POST /v1/operator/observer/replay`
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

## Transaction lifecycle

Use `GET /v1/receipt/{tx_hash_hex}` or
`GET /v1/tx/{tx_hash_hex}/receipt` for explorer-grade transaction status.
The response reports one of `pending`, `included`, `rejected`, `committed`, or
`finalized`, includes gas charged, safe rejection reason, withdrawal id when
present, deterministic typed `events`, and event-derived `contract_logs` for UI
display. Current consensus events include contract deploy, contract call, and
withdrawal creation; future contract-defined logs should remain bounded and
deterministic before becoming receipt data.

`GET /v1/block/{height}/finality` reports the L1 batch number, sanitized commit
status, sanitized finalization status, message hashes, and attempt counts. Raw
provider errors, signer details, and operator failure internals remain available
only under authenticated `/v1/operator/*` endpoints.

Postgres migrations run on startup and create tables for blocks, transactions,
receipts, deposits, withdrawals, L1 cursors, batch DA payloads, L1 batch commit
relays, L1 batch finalizations, observer checkpoints, and ENT faucet grants.

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

## Explorer and operator checks

Use the public and operator API endpoints directly from this L2 repository. Any
optional static frontend belongs outside the tracked L2 product tree, for example
under ignored local `ecosystem/` workspace files.

Contract links can be built from a deployment registry URL when one exists, for
example `deployments/testnet/entropis.json`, and opened in Tonviewer Testnet.

## Mempool admission limits

Public `POST /v1/tx` requests are admitted through a fail-closed mempool policy
before they reach the sequencer. The defaults are conservative for testnet and
can be tuned through environment variables:

```text
MEMPOOL_REPLAY_TTL_SECS=86400
MEMPOOL_NONCE_LOCK_TTL_SECS=300
MEMPOOL_LEADER_TTL_SECS=10
MEMPOOL_RATE_LIMIT_WINDOW_SECS=60
MEMPOOL_IP_RATE_LIMIT_WINDOW_SECS=60
MEMPOOL_MAX_GLOBAL_QUEUE=10000
MEMPOOL_MAX_ACCOUNT_QUEUE=64
MEMPOOL_MAX_ACCOUNT_NONCE_WINDOW=256
MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW=120
MEMPOOL_MAX_IP_SUBMISSIONS_PER_WINDOW=600
MEMPOOL_MAX_PAYLOAD_BYTES=16384
MEMPOOL_MAX_TRANSFER_PAYLOAD_BYTES=4096
MEMPOOL_MAX_WITHDRAW_PAYLOAD_BYTES=4096
MEMPOOL_MAX_CALL_PAYLOAD_BYTES=12288
MEMPOOL_MAX_DEPLOY_PAYLOAD_BYTES=16384
MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES=8192
MEMPOOL_MIN_GAS_LIMIT=1
MEMPOOL_MAX_GAS_LIMIT=1000000
MEMPOOL_MIN_GAS_PRICE=1
MEMPOOL_MAX_TX_FEE=1000000000000
MEMPOOL_POP_BATCH_SIZE=1024
MEMPOOL_BANNED_IPS=
MEMPOOL_BANNED_ACCOUNTS=
```

The mempool rejects duplicate transaction hashes, locked account nonces, malformed
signatures, wrong chain ids, zero/oversized gas policies, oversized public
payloads, per-kind payloads, oversized or malformed
`CallContract.body_boc_base64`, per-account queue floods, global queue floods,
wide pending nonce windows, banned accounts/IPs, and per-account/per-IP
rate-limit abuse. Bad-signature submissions with a valid sender/public-key pair
consume the same per-account and per-IP limits as valid submissions.
`GET /v1/mempool/metrics` exposes accepted/rejected reason counters, current
store queue depth, and eviction count for operators.

When the global queue is full, a new transaction is admitted only if its fee
priority is higher than the lowest-priority pending transaction, which is then
evicted. Block production pops transactions with deterministic account-fair
ordering: at most one pending transaction per account is selected per round,
with fee priority deciding the account order inside each round.

## Executor gas schedule

The executor uses ENT as the L2-native gas token. The gas coin asset id is `0`
(`L2_NATIVE_GAS_ASSET`), and fees are charged as `gas_used * max_gas_price` in
ENT base units. The executor uses a versioned gas schedule for
consensus-critical fee debits and receipt roots:

```text
EXECUTOR_GAS_SCHEDULE_VERSION=1
EXECUTOR_TRANSFER_GAS=10
EXECUTOR_WITHDRAW_GAS=20
EXECUTOR_CALL_CONTRACT_GAS=50
EXECUTOR_REJECTED_EXECUTION_GAS=1
EXECUTOR_MIN_GAS_PRICE=1
TVM_ADAPTER=real
TVM_TONLIB_LIBRARY_PATH=
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

`DeployContract` installs code/data/storage hashes for a new empty L2 contract
account and uses the same configured gas units as `CallContract`. It rejects zero
hashes and overwrites. `CallContract` requires `body_boc_base64` to decode into a
valid single-root TON BoC.

`TVM_ADAPTER=real` is the default. In this mode `CallContract` routes stored
code/data BoCs into the official TON `tonlibjson` TVM emulator through a
runtime-loaded native library boundary. Set `TVM_TONLIB_LIBRARY_PATH` when the
shared library is not discoverable from the platform library path. The adapter
does not read `.env`, filesystem, network, or wall clock during execution; all
deterministic C7/config fields come from the L2 execution context. Missing native
libraries, unsupported emulator results, malformed outputs, and unsupported
actions fail closed.

For local sample-counter demos without a native TVM library, set:

```text
TVM_ADAPTER=prototype
```

The prototype recognizes only the sample counter code hash, decodes the
Tolk-compatible `CounterIncrement` body, applies deterministic gas
`SAMPLE_COUNTER_INCREMENT_GAS=25`, and updates the sample storage root. Other code
hashes fail closed with `tvm_adapter_not_implemented`; malformed BoCs are rejected
earlier with `malformed_boc`.

Read-only contract getter requests do not create transactions and must not mutate
the L2 state root. The node exposes the live contract cell snapshot through:

```text
GET /v1/contract/{contract_id}/state
```

Getter calls use a bounded POST request:

```text
POST /v1/contract/{contract_id}/get-method
```

```json
{
  "method": "seqno",
  "method_id": null,
  "stack_boc_base64": null,
  "gas_limit": 100000
}
```

The response includes `read_only=true`, `state_root`, `method_id`, `gas_limit`,
`gas_used`, `vm_exit_code`, and a predictable result envelope. Built-in state
getters such as the sample counter and EnWallet V5 R1 getters return typed JSON.
Arbitrary contract getters require `TVM_ADAPTER=real` and return
`{"type":"vm_stack_boc","stack_boc_base64":"..."}` from the TON emulator stack.

Getter limits are runtime config, but not consensus state:

```text
TVM_GETTER_DEFAULT_GAS_LIMIT=100000
TVM_GETTER_MAX_GAS_LIMIT=1000000
TVM_GETTER_TIMEOUT_MS=500
TVM_GETTER_MAX_STACK_BOC_BYTES=16384
INTERNAL_QUEUE_MAX_LEN=4096
INTERNAL_QUEUE_MAX_PER_BLOCK=128
INTERNAL_MESSAGE_GAS_LIMIT=100000
```

Malformed method names, malformed stack BoCs, oversized stack payloads, and gas
limits above `TVM_GETTER_MAX_GAS_LIMIT` are rejected before TVM entry. The legacy
`GET /v1/contract/{id}/get/{method}` route remains for simple no-argument local
checks.

## Internal message queue

Contract calls can emit bounded async internal messages through the TVM adapter.
The sequencer appends those messages to a FIFO runtime queue and encodes each
delivered message as a system `InternalMessage` transaction in the next produced
blocks. Public `POST /v1/tx` rejects `InternalMessage`; only sequencer-created
system transactions may deliver queued contract-to-contract calls.

Ordering is deterministic:

- Public/system mempool transactions for the block are executed first.
- Only messages already pending at block start are eligible for delivery in that
  block.
- Messages emitted during a block are appended to the tail and become eligible in
  later blocks.
- Delivery is bounded by remaining block tx capacity,
  `INTERNAL_QUEUE_MAX_PER_BLOCK`, and the block gas limit.

Queue capacity is bounded by `INTERNAL_QUEUE_MAX_LEN`. If a contract tries to emit
more messages than the queue can hold, the originating transaction is rejected
with `internal_queue_full` and its state changes are rolled back. Adapter output
also remains capped by `max_internal_messages`, so a single contract cannot emit
unbounded messages even before queue admission.

Bounce handling follows the TON actor model at MVP level. A rejected bounceable
message schedules one bounced return message with `bounced=true`, `bounce=false`,
and a body beginning with opcode `0xffffffff`; bounced messages do not bounce
again. Non-zero internal message `value` is currently unsupported and fails
closed with `internal_value_not_supported`.

The queue is consensus-visible through DA because delivered internal messages are
part of the canonical batch transaction list. The node persists a queue snapshot
after each saved block and restores the latest snapshot during startup, so pending
deliveries survive a normal restart.

## L2 addresses

L2 account and sample contract ids are 32-byte ids internally. Public tooling
supports:

- raw technical addresses: `8:<64 lowercase hex chars>`
- user-friendly addresses: `EX...` deterministic base64url, 48 chars total;
  after `EX`, valid characters are `A-Z`, `a-z`, `0-9`, `-`, and `_`

Public account and sample-counter routes accept both formats, and the SDK accepts
legacy bare 64-hex values for compatibility with older tests and fixtures.

Sample counter local flow:

```powershell
$env:ENTROPIS_API_URL="http://127.0.0.1:8080"
$env:ENTROPIS_ADMIN_TOKEN="<local admin token>"
npm --prefix sdk run sandbox:l2-counter
```

To reset local L2 Postgres tables before the demo, stop the node first and run:

```powershell
.\scripts\demo\l2-counter-local.ps1 -Reset -ResetOnly
```

Then start `l2-node` again and run the sandbox command above.

The script generates a throwaway key, requests the local ENT faucet when an admin
token is present, deploys the sample counter code/data BoCs, submits an increment call,
produces local blocks through the admin endpoint, and reads `GET
/v1/sample-counter/{contract}`. It does not print the generated secret key.
Run the node with `TVM_ADAPTER=prototype` for this sample unless a working
`tonlibjson` emulator library is installed and `TVM_TONLIB_LIBRARY_PATH` points to
it.

Browser dApps should import `@ton-l2-rollup/sdk/browser`. That entrypoint exposes
`BrowserEntropisClient`, create/import helpers for 24-word EnWallet mnemonics,
transaction builders, contract deploy/call helpers, typed receipt parsing, and
public read/submit APIs. Admin-only faucet/deposit/block-production helpers live
under `@ton-l2-rollup/sdk/admin` and are intended for Node operator scripts or a
demo backend, not browser bundles.

## Data availability

The MVP stores canonical batch payload bytes in Postgres before saving a block as
pending for L1 relay:

```text
DA_MAX_PAYLOAD_BYTES=8388608
DA_PUBLIC_BACKEND=postgres
DA_PUBLIC_FS_DIR=build/da-public
DA_PUBLIC_BASE_URL=
```

The payload is the consensus `BatchData` bytes, not JSON. `data_hash` in the L2
block header is derived from those bytes. Before a batch is submitted to
`RollupRoot`, the relayer reads the payload back and rejects missing, corrupted,
partial, oversized, or wrong-block payloads without calling the signer or TON
provider.

For public retrievability in testnet prototype mode, use the filesystem gateway:

```text
DA_PUBLIC_BACKEND=filesystem
DA_PUBLIC_FS_DIR=build/da-public
DA_PUBLIC_BASE_URL=https://da.example.test/entropis
```

The node writes canonical payload files under:

```text
{DA_PUBLIC_FS_DIR}/blocks/{height}/{block_hash}-{data_hash}.el2batch
```

`DA_PUBLIC_BASE_URL` is optional. When set, the node stores a public URI alongside
the relative DA reference; it must point at an independently served mirror of
`DA_PUBLIC_FS_DIR`, not at private Postgres. Only filesystem payload files and the
Postgres mirror are written; bucket credentials, gateway tokens, and wallet files
must stay in `.env.local` or process environment and must not be mounted into the
public directory.

Operators and challengers can retrieve payload bytes through:

```text
GET /v1/da/batch/{height}
GET /v1/da/batch/{height}/{data_hash_hex}
```

The response body is `application/octet-stream` and includes
`x-entropis-block-height`, `x-entropis-block-hash`, `x-entropis-data-hash`, and
when configured `x-entropis-da-ref` / `x-entropis-da-uri`. The hash-specific route
is the safer replay path because it binds the payload request to the L1
`dataHash`. Future TON Storage support should implement the same `DaWriter`,
`DaReader`, and `DaVerifier` boundaries.

## Observer replay

The off-chain observer prototype is admin-only and does not post L1 challenges. It
accepts RollupRoot-shaped batch commitments from the caller, fetches canonical DA
bytes by `block_height + data_hash`, decodes transactions and receipts, replays
the deterministic executor from a trusted checkpoint, and reports the first
missing-DA, corrupt-DA, receipt, or root divergence.

```text
GET /v1/operator/observer/checkpoint
POST /v1/operator/observer/replay
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Replay request shape:

```json
{
  "trusted_checkpoint": null,
  "commitments": [
    {
      "batch_no": 1,
      "block_height": 0,
      "block_hash": "<l2 block hash>",
      "roots_a": {
        "prev_state_root": "<previous state root>",
        "state_root": "<claimed state root>",
        "tx_root": "<claimed tx root>"
      },
      "roots_b": {
        "receipt_root": "<claimed receipt root>",
        "withdrawal_root": "<claimed withdrawal root>",
        "data_hash": "<claimed DA hash>"
      }
    }
  ],
  "store_checkpoint": true
}
```

For the current prototype, commitments are supplied by the operator or a future
RollupRoot getter client; the observer must not derive them from local L2 block
JSON. Stored checkpoints include the replayed state snapshot and root so a later
bounded range can start from the last trusted point.

If replay returns `missing_da`, `corrupt_da`, or `invalid`, the response includes
`challenge_witness` when the finding maps to the future L1 challenge path. The
witness includes `l1_inputs.message = "ChallengeBatch"`, `challenge_kind_code`
(`1` for DA, `2` for invalid transition), optional field/tx index, expected and
claimed roots when applicable, checkpoint and commitment summaries, and an
`evidence_hash`. Treat it as off-chain evidence only: the current testnet root
does not yet accept challenge messages.

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
