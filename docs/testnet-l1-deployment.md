# Testnet L1 Deployment

This runbook deploys the Entropis TON L1 settlement pair:

- `RollupRoot`: batch commitments, challenge-window finalization, withdrawal claims.
- `AssetVault`: bridged TON custody, deposit logs, release execution.

The deployer wallet is also the initial admin for both contracts. The sequencer
address is supplied separately and must match the future batch signer sender.

## Why Linking Is Two-Step

TON contract addresses are derived from `StateInit` code and data. A root whose
initial data contains the vault address and a vault whose initial data contains
the root address would require a circular address fixed point. The script avoids
that impossible precompute:

1. Build `RollupRoot` with an explicit zero-address sentinel for `assetVault`.
2. Compute the root address from that state init.
3. Build `AssetVault` with the computed root address.
4. Deploy both contracts.
5. Send admin-only `SetAssetVault(vault)` to `RollupRoot`.

`SetAssetVault` is one-time only. It rejects unauthorized callers, the sentinel
address, already-linked roots, and roots that have already committed/finalized a
batch.

## Local Plan

Use WSL or Linux. Do not use `--net` for the local plan.

```bash
export L1_DEPLOYER_WALLET_LABEL=entropis-local-deployer
export L1_DEPLOY_OUTPUT_JSON=build/testnet-l1-deployment.json

acton run l1-deploy-plan -- \
  <sequencer-address> \
  <wrapped-gas-minter-address> \
  300 \
  1 \
  9
```

Arguments are:

- `sequencer-address`: authorized `RollupRoot` batch sender.
- `wrapped-gas-minter-address`: placeholder/minter address stored in `AssetVault`.
- `300`: testnet challenge window in seconds.
- `1`: bridged TON asset id.
- `9`: bridged TON decimals.

The script prints planned addresses, deploys in emulation, links the root to the
vault, verifies getters, and writes JSON to `L1_DEPLOY_OUTPUT_JSON`. The default
path is under ignored `build/`. Keep raw deployment outputs ignored; only the
curated public registry files under `deployments/testnet/` are tracked.

## Testnet Deploy

Create or import a funded testnet deployer wallet locally. Do not commit
`wallets.toml`, mnemonics, wallet exports, or `.env.local`.

```bash
acton wallet new --name entropis-testnet-deployer --local --airdrop --version v5r1 --secure true
acton wallet list --balance
```

Then run the same script with the testnet alias:

```bash
export L1_DEPLOYER_WALLET_LABEL=entropis-testnet-deployer
export L1_DEPLOY_OUTPUT_JSON=build/testnet-l1-deployment.json

acton run l1-deploy-testnet -- \
  <sequencer-address> \
  <wrapped-gas-minter-address> \
  300 \
  1 \
  9
```

This alias expands to `acton script ... --net testnet --explorer tonviewer`.
There is no mainnet alias. Do not run this script with `--net mainnet`.

The script refuses replay when either planned contract address is already
deployed. The root link is also one-time on-chain, so replay cannot overwrite an
existing root-to-vault link. It also fails closed when `TON_NETWORK=mainnet` is
present in the environment; leave `TON_NETWORK` unset or set it to `testnet`.

## Readback

After deployment, verify the getter state against the output JSON:

```bash
acton run l1-verify-testnet -- \
  <rollup-root-address> \
  <asset-vault-address> \
  <admin-address> \
  <sequencer-address> \
  <wrapped-gas-minter-address> \
  300 \
  1 \
  9
```

The verify alias runs with `--fork-net testnet`, so it reads testnet state without
broadcasting transactions.

Expected getter values:

- `RollupRoot.rollupStatus().assetVault == AssetVault address`
- `RollupRoot.rollupStatus().sequencer == sequencer-address`
- `RollupRoot.rollupStatus().challengeWindowSec == 300`
- `RollupRoot.rollupStatus().lastCommitted == 0`
- `RollupRoot.rollupStatus().lastFinalized == 0`
- `RollupRoot.rollupStatus().paused == false`
- `AssetVault.vaultStatus().rollupRoot == RollupRoot address`
- `AssetVault.vaultStatus().wrappedGasMinter == wrapped-gas-minter-address`
- `AssetVault.vaultStatus().tonAssetId == 1`
- `AssetVault.vaultStatus().tonDecimals == 9`
- `AssetVault.vaultStatus().lockedTon == 0`
- `AssetVault.vaultStatus().paused == false`

## Public Registry Update

After the testnet deploy and getter readback pass, update
`deployments/testnet/entropis.json` from the ignored deployment output:

- move the deployment status from `draft` to `verified`;
- set `activeDeploymentId` to the verified deployment id;
- add `RollupRoot` and `AssetVault` addresses, code hashes, data hashes, and
  deploy transaction hashes;
- add deployer public address, sequencer address, challenge window, wrapper
  generation commit, deployed timestamp, and getter verification evidence;
- leave API keys, signer tokens, wallet files, provider endpoints, and database
  URLs out of the registry.

Validate the registry before staging:

```bash
python scripts/ci/validate_deployment_registry.py deployments/testnet/entropis.json
```

The registry is the public source of truth for SDK examples, operator runbooks,
and node address references. Deprecated deployments should remain in the file
with `status: "deprecated"` instead of being overwritten.

## Node Config Sync

Copy the deployed values from the ignored JSON into local runtime config:

```text
L1_ROLLUP_ROOT_ADDRESS=<rollupRoot>
L1_VAULT_ADDRESS=<assetVault>
L1_SEQUENCER_SENDER_ADDRESS=<sequencer>
L1_TON_ASSET_ID=1
L1_DEPOSIT_ASSET_IDS=1
L2_CHALLENGE_WINDOW_SEC=300
```

Keep deployment secrets and wallet material outside tracked files.
