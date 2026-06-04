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
- `acton test`: passed, 18 tests
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

## Findings

### No Validated Critical Or High Issues

The tested L1 MVP behavior did not expose a validated critical or high severity exploit in the currently enabled paths.

### Resolved: Withdrawal Proof Verification Is Enabled

`verifyWithdrawalProof` now verifies compact Merkle proofs over `ReleaseAuthorized` TON-cell representation hashes. `RollupRoot` also binds `ClaimWithdrawal.withdrawalId` to the decoded `ReleaseAuthorized.withdrawalId` before marking the claim and sending the release message.

Status: implemented with positive and adversarial Acton tests.

### Medium: Bounce Recovery Requires More Coverage Before Real Withdrawals

`RollupRoot` and `AssetVault` both define bounced-message handling for release paths. Valid claim paths are now enabled, so bounce recovery still needs end-to-end coverage with failed recipient delivery and retry semantics before production funds.

Status: residual production-readiness risk for the next bridge milestone.

### Low: Permissionless Finalization Is Explicit But Should Stay Documented

`FinalizeBatch` does not require the sequencer sender. This is acceptable for an optimistic rollup flow because finalization after the challenge window can be permissionless. The behavior should remain intentional and documented if challenge logic changes.

Status: accepted design.

## Required Next Tests Before Production Withdrawals

- Vault release bounce to failed recipient and retry.
- Root bounce authorization for failed vault release messages.
- Jetton/wrapped-gas release authorization and wallet identity checks.
- Gas measurements for proof depths greater than one chunk.
