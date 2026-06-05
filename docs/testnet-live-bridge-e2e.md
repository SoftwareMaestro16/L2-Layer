# Live Testnet Bridge E2E

This runbook closes the TON testnet bridge loop only after the public registry is
verified. It is not a mainnet readiness document. Keep deployer wallets, signer
tokens, provider keys, database URLs, Redis URLs, mnemonics and signed BoCs out
of this file.

## Current Gate

The tracked registry lives at `deployments/testnet/entropis.json`. A live run is
blocked while the registry status is `draft`. Promote it only after:

- `RollupRoot` and `AssetVault` are deployed on TON testnet.
- `acton run l1-verify-testnet -- ...` matches the registry values.
- `scripts/ci/validate_deployment_registry.py deployments/testnet/entropis.json`
  passes.
- `.env.local` contains the public root/vault addresses and local-only secrets.

## Non-Secret Wiring

Use these public values from the active registry deployment:

```text
TON_NETWORK=testnet
L2_CHAIN_ID=entropis-testnet
L1_ROLLUP_ROOT_ADDRESS=<registry RollupRoot address>
L1_VAULT_ADDRESS=<registry AssetVault address>
L1_SEQUENCER_SENDER_ADDRESS=<registry sequencer address>
L1_TON_ASSET_ID=1
L1_DEPOSIT_ASSET_IDS=1
L2_CHALLENGE_WINDOW_SEC=<registry challengeWindowSec>
L1_DEPOSIT_INDEXER_ENABLED=true
L1_BATCH_RELAYER_ENABLED=true
L1_BATCH_FINALIZER_ENABLED=true
L2_DEV_ADMIN_DEPOSITS_ENABLED=false
```

Secrets stay in `.env.local` or operator secret storage:
`TONCENTER_API_KEY`, `TONAPI_KEY`, `DATABASE_URL`, `REDIS_URL`,
`L2_ADMIN_TOKEN`, signer tokens and wallet material.

## Flow

1. Verify registry and launch gates.
2. Start Postgres, Redis, signer and `l2-node`.
3. Confirm `/readyz`, `/v1/operator/batch-relayer` and
   `/v1/operator/batch-finalizer`.
4. Send `DepositTon` to `AssetVault` with `scripts/l1/deposit_ton.tolk`.
5. Wait for the indexer to show `GET /v1/explorer/deposit/{id}`.
6. Verify the credited L2 balance through `GET /v1/account/{id}`.
7. Submit a signed L2 transfer and wait for inclusion.
8. Verify DA by `GET /v1/da/batch/{height}/{data_hash}`.
9. Wait for relayer confirmation and compare `RollupRoot.commitment(batchNo)`.
10. Wait for finalizer confirmation after `challengeWindowSec`.
11. Submit an L2 withdrawal, fetch `GET /v1/proof/withdrawal/{id}` after
    finalization, build a `ClaimWithdrawal` body and send it to `RollupRoot`.
12. Confirm `AssetVault` releases TON and locked TON decreases by the amount.

## Negative Checks

Run these before public demo sign-off:

| Case | Expected result |
| --- | --- |
| Forged deposit log from another source | Indexer rejects; cursor does not credit |
| Duplicate deposit event | Storage idempotency prevents second credit |
| Wrong vault address in config | `/readyz` or indexer status reports failure |
| Missing or corrupt DA | Relayer refuses signing and records safe error |
| Wrong signer address | Signer client/root authorization fails closed |
| Duplicate finalization | Root rejects; finalizer does not loop forever |
| Duplicate withdrawal claim | Root rejects using `claimedWithdrawals` |

## Public Evidence

Record only public-safe evidence:

| Step | Evidence |
| --- | --- |
| Registry | commit hash, registry status, root/vault addresses, code hashes |
| Deposit | Tonviewer testnet URL, deposit id, credited account, amount |
| Transfer | tx hash, block height, receipt status |
| DA | block height, data hash, public DA response hash |
| Commit | batch number, commitment getter values, Tonviewer URL |
| Finalize | finalized batch number, finalizer status, Tonviewer URL |
| Withdraw | withdrawal id, proof status, claim URL, vault locked TON delta |

Do not record private endpoints, API keys, bearer tokens, raw provider JSON,
mnemonics, wallet exports or raw signed BoCs.
