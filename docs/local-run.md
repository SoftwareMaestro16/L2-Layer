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
- `GET /v1/proof/withdrawal/{withdrawal_id_hex}`
- `WS /v1/stream`

`POST /v1/admin/deposit` is a local-development adapter and only works when
`L2_DEV_ADMIN_DEPOSITS_ENABLED=true`. In production/testnet flows, deposits should
come from the TON deposit indexer. Admin endpoints require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Postgres migrations run on startup and create tables for blocks, transactions,
receipts, deposits, withdrawals, L1 cursors, and ENT faucet grants.

The ENT faucet is L2-native only in this phase. It grants `ENT_FAUCET_AMOUNT`
whole ENT per account, converted with `ENT_DECIMALS=9`, and requires the admin
bearer token until public rate limiting is implemented.

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
```

It polls Toncenter v3 `/messages` for `DepositRecorded` external logs emitted by
the configured vault, stores progress in `l1_cursors`, saves deposits idempotently,
and feeds new deposits into the sequencer. Malformed expected logs fail closed and
do not advance the cursor.

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
The latest release checked during implementation was `v1.1.0`; its release assets
publish Linux/macOS archives and a shell installer, but no native Windows archive.

Expected commands once Acton is available:

```powershell
acton --version
acton doctor
acton build
acton test
acton wrapper RollupRoot --ts
acton wrapper AssetVault --ts
```
