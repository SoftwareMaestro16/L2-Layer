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

The two admin endpoints are local-development adapters for the missing TON indexer
and background relayer in this first scaffold. They require:

```text
Authorization: Bearer <L2_ADMIN_TOKEN>
```

Postgres migrations run on startup and create tables for blocks, transactions,
receipts, deposits, withdrawals, L1 cursors, and ENT faucet grants.

The ENT faucet is L2-native only in this phase. It grants `ENT_FAUCET_AMOUNT`
whole ENT per account, converted with `ENT_DECIMALS=9`, and requires the admin
bearer token until public rate limiting is implemented.

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
