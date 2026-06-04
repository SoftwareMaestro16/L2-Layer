# Testnet Runtime Profile

This runbook starts `l2-node` in the live `testnet-prototype` profile. It uses
real Postgres, Redis, TON testnet providers, verified `RollupRoot` and
`AssetVault` addresses, and an external signer endpoint. It does not use local
admin deposit shortcuts.

## Source Of Truth

Use `deployments/testnet/entropis.json` for public deployment metadata:

- `RollupRoot` address and code hash.
- `AssetVault` address and code hash.
- challenge window, currently `300` seconds.
- TON asset id and decimals, currently asset id `1` and `9` decimals.
- deployment status and getter verification evidence.

The registry may be `draft` before real deployment. Do not start
`testnet-prototype` until a deployment is marked `verified` and copied into
local runtime config.

## Required `.env.local`

Start from `.env.example`, keep `.env.local` ignored, and set:

```text
L2_RUNTIME_MODE=testnet-prototype
L2_DEV_ADMIN_DEPOSITS_ENABLED=false
L2_CHAIN_ID=entropis-testnet
L2_CHALLENGE_WINDOW_SEC=300
ENT_DECIMALS=9
ENT_FAUCET_REQUIRE_ADMIN=true

TON_NETWORK=testnet
TONCENTER_V3_BASE_URL=https://testnet.toncenter.com/api/v3
TONCENTER_API_KEY=<toncenter testnet key>
TONAPI_BASE_URL=https://testnet.tonapi.io
TONAPI_KEY=<tonapi testnet key>

DATABASE_URL=<postgresql URL from operator secrets>
REDIS_URL=<redis URL from operator secrets>
L2_ADMIN_TOKEN=<random admin bearer token>

L1_VAULT_ADDRESS=<verified AssetVault address from registry>
L1_ROLLUP_ROOT_ADDRESS=<verified RollupRoot address from registry>
L1_TON_ASSET_ID=1
L1_DEPOSIT_ASSET_IDS=1
L1_DEPOSIT_INDEXER_ENABLED=true

L1_BATCH_RELAYER_ENABLED=true
L1_SEQUENCER_SENDER_ADDRESS=<RollupRoot sequencer address>
L1_COMMIT_SIGNER_ENDPOINT=<operator signer endpoint>
L1_COMMIT_SIGNER_TOKEN=<random signer bearer token>
```

The live profile defaults the deposit indexer and batch relayer to enabled, and
defaults admin deposits to disabled. The node refuses `testnet-prototype` if
admin deposits are enabled, if either live worker is disabled, or if required L1
addresses and signer settings are missing.

## Startup Checklist

1. Validate the public registry:

```powershell
python scripts/ci/validate_deployment_registry.py deployments/testnet/entropis.json
```

2. Start Postgres and Redis, then confirm credentials are only in `.env.local`.
3. Start the external signer service and check its safe health endpoint.
4. Copy the verified root/vault addresses and challenge window from the registry.
5. Run the node:

```powershell
cargo run -p l2-node
```

Postgres migrations run on startup before the API is served. Startup logging uses
a safe summary: runtime mode, public testnet endpoints, enabled flags, public
contract addresses, and limits. It does not include API keys, admin tokens,
signer tokens, database URLs, Redis URLs, wallet material, or raw BoCs.

## Readiness And Operations

Use:

```powershell
Invoke-RestMethod http://127.0.0.1:8080/readyz
```

`/readyz` reports `db`, `redis`, and `ton` component status with safe reason
codes only. Use authenticated `GET /v1/operator/metrics` for counters and
latency snapshots. Admin endpoints require `Authorization: Bearer
<L2_ADMIN_TOKEN>` and should not be exposed without an authenticated reverse
proxy.

After the node is ready, use the live TON deposit rehearsal in
`docs/testnet-ton-deposit-e2e.md` to send a testnet wallet deposit, observe the
Toncenter v3 log, and confirm the L2 account balance without admin deposit
shortcuts.

For token rotation, stop the node or remove it from public traffic, update
`.env.local`, restart the signer if `L1_COMMIT_SIGNER_TOKEN` changed, then restart
`l2-node`. Rotate `L2_ADMIN_TOKEN` and signer tokens independently. Provider API
keys can be rotated without changing the public registry.

## Failure Checks

- Mainnet endpoints are rejected by config validation.
- Missing `L1_VAULT_ADDRESS` fails when the indexer is enabled.
- Missing root, sequencer sender, signer endpoint, or signer token fails when
  the relayer is enabled.
- `L2_DEV_ADMIN_DEPOSITS_ENABLED=true` fails in `testnet-prototype`.
- `L2_CHALLENGE_WINDOW_SEC=0` fails; any non-zero value must still match the
  verified registry and L1 getter readback operationally.
- Logs and readiness responses must never include secrets from `.env.local`.
