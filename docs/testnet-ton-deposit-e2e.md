# Testnet TON Deposit E2E

This runbook proves the live TON deposit path:

```text
user wallet -> AssetVault.DepositTon -> DepositRecorded log
  -> Toncenter v3 /messages -> l2-node indexer
  -> storage deposit idempotency -> sequencer system deposit
  -> L2 account balance
```

It never uses `POST /v1/admin/deposit` and never stores wallet credentials in
the repository.

## Prerequisites

- `deployments/testnet/entropis.json` has a verified `AssetVault` address.
- `l2-node` runs with `L2_RUNTIME_MODE=testnet-prototype`.
- `L1_DEPOSIT_INDEXER_ENABLED=true`.
- `L1_VAULT_ADDRESS` matches the verified registry vault address.
- `L1_DEPOSIT_ASSET_IDS` includes `1` for bridged TON.
- A funded TON testnet wallet can send a raw payload transfer.
- The L2 recipient is a 32-byte account id hex string.

## Build The Wallet Payload

Build the SDK once, then create a TON Connect request:

```powershell
Set-Location sdk
npm ci
npm run build
Set-Location ..

$env:L1_VAULT_ADDRESS = "<verified AssetVault address>"
$env:L2_RECIPIENT = "<32-byte L2 account id hex>"
$env:DEPOSIT_QUERY_ID = "1"
$env:DEPOSIT_AMOUNT_NANOTON = "100000000"
$env:DEPOSIT_MSG_VALUE_NANOTON = "110000000"
node scripts/testnet/build-deposit-ton-transfer.mjs > tmp/deposit-ton-transfer.json
```

`DEPOSIT_AMOUNT_NANOTON` is the locked bridge amount. `DEPOSIT_MSG_VALUE_NANOTON`
must be greater than or equal to it and can include extra execution fee headroom.

The generated JSON contains:

- `tonConnectRequest.network = "-3"` for TON testnet.
- `tonConnectRequest.messages[0].address = AssetVault`.
- `tonConnectRequest.messages[0].amount = message value in nanotons`.
- `tonConnectRequest.messages[0].payload = DepositTon body as base64 BoC`.

Send that request from the user wallet. Record the public wallet transaction hash
and the resulting `DepositRecorded` log hash.

## Observe Toncenter

Toncenter v3 must expose a log message from the vault:

```powershell
curl "https://testnet.toncenter.com/api/v3/messages?source=$env:L1_VAULT_ADDRESS&destination=null&opcode=0x4c324407&start_lt=1&limit=10&sort=asc" `
  -H "X-API-Key: $env:TONCENTER_API_KEY"
```

Expected decoded fields:

- `queryId` matches the payload request.
- `depositId` is non-zero.
- `assetId` is `1`.
- `amount` matches `DEPOSIT_AMOUNT_NANOTON`.
- `l2Recipient` matches the L2 account id.
- `source` is exactly the configured vault.
- `destination` is `null`.

## Observe The Node

Check readiness and indexer counters:

```powershell
curl http://127.0.0.1:8080/readyz
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/metrics
```

`node.indexer.accepted` should increase by one after the log is indexed.
`node.indexer.duplicates` may increase on replay, but the L2 balance must not
increase twice.

Then verify the user balance without an admin token:

```powershell
curl http://127.0.0.1:8080/v1/account/$env:L2_RECIPIENT
```

The account balance for asset id `1` should increase by the deposit amount after
the sequencer produces the next block.

## Negative Checks

- Change `L1_VAULT_ADDRESS` to another address and confirm the parser rejects the
  log as `deposit log source is not vault`.
- Replay the same Toncenter message or restart the node; duplicate `depositId` or
  duplicate `(l1_tx_hash, l1_lt)` must not credit again.
- Use a malformed decoded body; the poll must fail without advancing the cursor.
- Use an unsupported `assetId`; the event must be rejected.
- Submit a system deposit through public `POST /v1/tx`; the mempool must reject
  it before execution.

## Optional Live Smoke Test

The ignored Rust smoke test fetches Toncenter v3 logs and validates every returned
`DepositRecorded` payload with the same parser used by the runtime indexer:

```powershell
$env:ENTROPIS_LIVE_TON_DEPOSIT="1"
$env:L1_VAULT_ADDRESS="<verified AssetVault address>"
$env:TONCENTER_API_KEY="replace-with-testnet-provider-key"
$env:ENTROPIS_LIVE_TON_DEPOSIT_START_LT="1"
cargo test -p l2-node --test ton_deposit_fixture live_toncenter_deposit_indexer_smoke_requires_env -- --ignored
```

Do not paste API keys, wallet seeds, signed BoCs, database URLs, Redis URLs, or
admin tokens into the runbook, Git commits, or public logs.

## References

- TON Connect raw transaction messages use `{ address, amount, payload }` with
  base64 BoC payloads and testnet network id `-3`.
- Toncenter v3 `/messages` supports `source`, `destination=null`, `opcode`,
  `start_lt`, `limit`, and `sort=asc` filters for log polling.
- TON BoC is the canonical cell serialization format used for the deposit body.
