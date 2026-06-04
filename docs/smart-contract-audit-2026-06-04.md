# Smart Contract Audit - 2026-06-04

Scope:
- `contracts/l1/rollup-root/rollup_root.tolk`
- `contracts/l1/asset-vault/asset_vault.tolk`
- shared L1 message, storage, proof, and error schemas
- Acton wrapper-driven contract tests under `tests/`

## Tooling

- `acton fmt --check`: passed
- `acton build`: passed
- `acton check`: passed
- `acton test`: passed, 22 tests
- `npx tsa-installer install`: failed locally with `npm ERR! cb.apply is not a function`; TSA analysis was unavailable in this environment.

## Coverage Added

Security/adversarial tests were added in `tests/contracts_security.test.tolk`.

Covered properties:
- RollupRoot rejects unauthorized batch commits.
- RollupRoot enforces monotonic batch numbers.
- RollupRoot rejects stale previous state roots.
- RollupRoot rejects missing batch finalization.
- RollupRoot rejects duplicate finalization.
- RollupRoot ignores empty top-ups and rejects unknown non-empty bodies with `Errors.UnknownOpcode`.
- AssetVault rejects underfunded TON deposits without changing locked accounting.
- AssetVault rejects unauthorized releases without changing locked accounting.
- AssetVault debits locked TON exactly on authorized TON release.
- AssetVault does not debit locked TON for unsupported asset release.
- AssetVault ignores empty top-ups and rejects unknown non-empty bodies with `Errors.UnknownOpcode`.
- RollupRoot accepts a valid finalized withdrawal proof.
- RollupRoot rejects duplicate withdrawal claims.
- RollupRoot rejects withdrawal claims before batch finalization.
- RollupRoot rejects wrong roots, mismatched withdrawal ids, and corrupted sibling order.
- RollupRoot records bounced vault releases and allows permissionless retry without reopening the claim.
- RollupRoot rejects bounced release status creation when the withdrawal was not previously claimed.
- AssetVault records recipient release bounces, re-credits TON locked accounting, and allows one successful retry.
- AssetVault rejects retry for unsupported asset failures and rejects spoofed recipient bounces.

## Findings

### No Validated Critical Or High Issues

The tested L1 MVP behavior did not expose a validated critical or high severity exploit in the currently enabled paths.

### Resolved: Withdrawal Proof Verification Is Enabled

`verifyWithdrawalProof` now verifies compact Merkle proofs over `ReleaseAuthorized` TON-cell representation hashes. `RollupRoot` also binds `ClaimWithdrawal.withdrawalId` to the decoded `ReleaseAuthorized.withdrawalId` before marking the claim and sending the release message.

Status: implemented with positive and adversarial Acton tests.

### Resolved: Bounce Recovery And Retry Are Implemented For TON Releases

`RollupRoot` stores root-to-vault failures without clearing `claimedWithdrawals`, so a bounced release cannot reopen the proof claim path. `AssetVault` now sends `ReleaseAuthorized` metadata in recipient release messages, records recipient bounces, re-credits `lockedTon` for TON asset failures, and exposes permissionless retry from stored failure records.

Status: implemented for TON asset releases with adversarial Acton tests. Jetton/wrapped-gas release remains intentionally unsupported.

### Low: Permissionless Finalization Is Explicit But Should Stay Documented

`FinalizeBatch` does not require the sequencer sender. This is acceptable for an optimistic rollup flow because finalization after the challenge window can be permissionless. The behavior should remain intentional and documented if challenge logic changes.

Status: accepted design.

## Required Next Tests Before Production Withdrawals

- Vault release bounce to failed recipient and retry.
- Root bounce authorization for failed vault release messages.
- Gas measurements for proof depths greater than one chunk.
- Jetton/wrapped-gas release authorization and wallet identity checks.
- End-to-end testnet withdrawal claim through deployed RollupRoot and AssetVault.
