# Security Audit - Testnet Prototype - 2026-06-04

## Scope

This pass reviewed the testnet prototype security boundary across:

- L1 Tolk contracts: `RollupRoot`, `AssetVault`, shared message, storage, proof, and error schemas.
- L2 Rust runtime: deposit indexer, sequencer admission, executor, DA, relayer, finalizer, observer, API, storage, and mempool.
- Signer boundary: typed signer requests, signer service auth, role/action routing, response validation, and retry behavior.
- Operational controls: `.env.local`, tracked examples, CI secret/artifact guards, logs, deployment outputs, and operator runbooks.

References consulted:

- TON Docs `blockchain-basics/contract-dev/techniques/security`
- TON Docs `blockchain-basics/primitives/messages/modes`
- TON Docs `blockchain-basics/standard/tokens/jettons/how-it-works`
- Repository audit guidance in `AGENTS.md` and the TON smart-contract audit checklist.

## Severity

- Critical: direct loss of bridged funds or unrestricted contract drain.
- High: exploitable double release, forged credit, sequencer/signing bypass, or secret exposure.
- Medium: fund lock, incorrect crediting, replay or liveness failure requiring operator action.
- Low: documented trust, availability, or hardening gaps without immediate fund loss.

## Findings

### No Open Critical Or High Issues Validated

No validated critical or high issue remains open for the testnet prototype scope reviewed in this pass.

### Medium - Resolved: Reused On-Chain Deposit Id Could Suppress Distinct L1 Deposits

`DepositRecorded.depositId` was derived on-chain from sender, query id, asset id, amount, and L2 recipient. A user could send two real TON deposits with the same tuple in different transactions. The indexer used that field as the L2 `deposit_id`, so storage treated the second real event as a duplicate even though `(l1_tx_hash, l1_lt)` differed.

Fix:

- `parse_deposit_message` now validates the on-chain event id is non-zero, then derives the credited L2 `deposit_id` from vault source, TON message hash, logical time, and the event id.
- Replay protection still rejects the same `(l1_tx_hash, l1_lt)` twice.

Evidence:

- `crates/l2-node/src/indexer.rs`
- `crates/l2-node/src/indexer_tests.rs` test `repeated_contract_deposit_id_with_new_l1_identity_is_credited`

### Medium - Resolved: Native TON Deposit Accepted Zero L2 Recipient On L1

The Rust indexer rejected zero L2 recipients, but `AssetVault.DepositTon` accepted them and locked TON before emitting a log that the indexer would refuse to credit. This was not a forged-credit path, but it could lock user funds in an unusable deposit.

Fix:

- `AssetVault.handleDepositTon` rejects `l2Recipient == 0`.

Evidence:

- `contracts/l1/asset-vault/asset_vault.tolk`
- `tests/contracts_security.test.tolk` test `test security vault rejects zero l2 recipient deposit`

### Low - Resolved: RollupRoot Could Accept Commit Before Vault Link

`SetAssetVault` intentionally rejects linking after any batch is committed. Before this pass, the sequencer could commit batch 1 while `assetVault` still held the zero sentinel, leaving the root un-linkable. This required sequencer/operator misconfiguration and was not externally exploitable, but it could brick a deployment.

Fix:

- `RollupRoot.handleCommitBatch` now rejects commits until `assetVault` is linked.

Evidence:

- `contracts/l1/rollup-root/rollup_root.tolk`
- `tests/deployment_linking.test.tolk` test `test deployment rejects batch commit before root vault link`

## Reviewed Controls

L1 contracts:

- `CommitBatch` is sequencer-only, monotonic by batch number, and checks previous state root after batch 1.
- `FinalizeBatch` is permissionless by design after `challengeWindowSec`; early, missing, and duplicate finalization are tested.
- Withdrawal claims require finalized commitments, compact Merkle proof verification, withdrawal-id binding, and `claimedWithdrawals` before async release.
- Root-to-vault and vault-to-recipient bounces keep claims closed and store retry records.
- Jetton deposits accept `transfer_notification` only from registered vault-owned Jetton wallet addresses and reject malformed/zero-recipient forward payloads.

L2 runtime:

- User transactions bind signatures to chain id, nonce, sender-derived account id, gas limits, and typed transaction payloads.
- System deposit transactions cannot enter the public mempool path.
- Mempool duplicate tx, nonce lock, per-account queue, global queue, payload size, and rate limits have tests.
- DA writes canonical batch bytes and relayer verifies hash, block hash, and public retrievability before signing.
- Observer replay uses DA payloads and trusted checkpoints, not local sequencer block JSON, and reports missing DA separately from invalid roots.

Relayer, finalizer, and signer:

- Signer service requires bearer auth, bounded body size, request validity, role/action allowlist, optional rollup-root binding, signer-address match, valid BoC shape, and rate limiting.
- Relayer rejects missing DA, block hash mismatch, signer mismatch, expired/malformed/oversized BoC, and uses bounded attempts.
- Finalizer creates pending work only from confirmed commits, waits local confirmation time plus `challengeWindowSec`, validates signer response, and uses bounded attempts.

Operational secrets:

- Runtime secrets belong only in `.env.local` or process environment.
- `NodeConfig` debug output redacts secret fields.
- `/readyz` returns safe component codes without DB, Redis, TON API, admin, or signer secrets.
- CI guard scripts block tracked/staged live secrets, local env files, wallets, build outputs, deployment output JSON, and local roadmaps.

## Residual MVP Risks

- No on-chain fraud proof exists yet; the observer/challenger is off-chain detection only.
- Public DA filesystem mode improves retrievability but is not a censorship-resistant DA network.
- `CallContract` remains fail-closed until the real TVM adapter is integrated; future adapter work needs a separate nondeterminism and resource-limit audit.
- Admin and operator endpoints rely on bearer tokens and deployment topology. Do not expose them publicly without an authenticated reverse proxy.
- `CorsLayer::permissive()` is acceptable for local browser tooling only while admin tokens are never stored in browser bundles or local storage.
- Permissionless finalization is intentional for the optimistic MVP, but challenge logic must revisit it before production withdrawals.
- TSA symbolic analysis was not required to validate the resolved Rust/indexer findings; if TSA is available in CI, add `RollupRoot` and `AssetVault` drain/bounce checks as a later quality gate.

## Manual Testnet Attack Checklist

Run this before a public demo on deployed testnet addresses:

- Verify `RollupRoot.rollupStatus().assetVault == AssetVault.address` before enabling the relayer.
- Attempt `CommitBatch` from a non-sequencer wallet and confirm rejection.
- Attempt `CommitBatch` on an unlinked local/fork deployment and confirm rejection.
- Attempt `FinalizeBatch` before `challengeWindowSec` and confirm rejection.
- Submit duplicate `FinalizeBatch` and confirm rejection or no repeated status change.
- Send a zero-recipient `DepositTon` body and confirm vault rejection.
- Send two real deposits with the same query id, amount, and recipient; confirm both distinct L1 events credit L2 once each.
- Replay the same Toncenter `lt/hash` fixture and confirm no second L2 credit.
- Send a forged `DepositRecorded`-shaped log from a non-vault source and confirm indexer rejection.
- Send a fake Jetton `transfer_notification` from an unregistered wallet and confirm rejection.
- Submit a bad signer response with wrong signer address, expired validity, malformed BoC, and wrong action route; confirm no Toncenter broadcast.
- Make DA unavailable or corrupted for a pending batch and confirm the relayer does not request a signature.
- Submit `ClaimWithdrawal` before finalization, with wrong root, mismatched withdrawal id, corrupted sibling order, and duplicate claim; confirm rejection.
- Trigger root-to-vault and vault-to-recipient bounce paths on fork/local test and confirm retry records do not reopen double claims.

## Validation Status

Completed in this branch:

- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed: 44 `l2-core` tests and 120 `l2-node` tests passed, with 1 ignored live-testnet finalizer smoke.
- `npm ci` from `sdk` passed.
- `npm run typecheck` from `sdk` passed.
- WSL Acton 1.1.0 `acton doctor` completed. Native PowerShell Acton is unavailable.
- WSL `acton build`, `acton test`, `acton check`, and `acton fmt --check` passed: 35 contract tests passed.

Local validation notes:

- The first `cargo test --workspace` attempt failed with `rustc-LLVM ERROR: IO failure on output stream: no space on device`. The repo-local `target/` directory was verified under the workspace and cleaned with `cargo clean`; the retry passed.
- WSL prints `Failed to translate 'D:\MongoDB\Server\5.0\bin'`, but Acton commands complete successfully.
- TSA symbolic analysis was not run. `npx tsa-installer location` failed locally with `cb.apply is not a function` before returning a TSA CLI path. The validated findings in this report are proven by repository tests.

Required immediately before commit:

- `python scripts/ci/secret_scan.py --staged`
- `python scripts/ci/artifact_guard.py --staged`
