# Local Run

## Rust node

```powershell
cargo run -p l2-node
```

Useful endpoints:

- `POST /v1/tx`
- `POST /v1/admin/deposit`
- `POST /v1/admin/produce-block`
- `GET /v1/account/{account_id_hex}`
- `GET /v1/block/{height}`
- `GET /v1/tx/{tx_hash_hex}`
- `GET /v1/proof/withdrawal/{withdrawal_id_hex}`
- `WS /v1/stream`

The two admin endpoints are local-development adapters for the missing TON indexer
and background relayer in this first scaffold.

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
