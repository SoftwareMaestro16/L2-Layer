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
- `acton test`: passed, 14 tests
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

## Findings

### No Validated Critical Or High Issues

The tested L1 MVP behavior did not expose a validated critical or high severity exploit in the currently enabled paths.

### Medium: Withdrawal Claims Are Intentionally Fail-Closed

`verifyWithdrawalProof` currently returns `false`, so `ClaimWithdrawal` cannot release funds. This blocks withdrawals, but prevents forged withdrawal proofs until the real verifier is implemented.

Status: accepted MVP limitation. Do not enable production withdrawals until positive and negative Merkle proof tests exist.

### Medium: Bounce Recovery Requires More Coverage Before Real Withdrawals

`RollupRoot` and `AssetVault` both define bounced-message handling for release paths, but the current proof verifier prevents real claims from reaching the release path. Before enabling valid withdrawal proofs, bounce recovery must be tested end-to-end, including failed recipient delivery and retry semantics.

Status: residual risk for the next bridge milestone, not currently exploitable through `ClaimWithdrawal` because claims fail closed.

### Low: Permissionless Finalization Is Explicit But Should Stay Documented

`FinalizeBatch` does not require the sequencer sender. This is acceptable for an optimistic rollup flow because finalization after the challenge window can be permissionless. The behavior should remain intentional and documented if challenge logic changes.

Status: accepted design.

## Required Next Tests Before Enabling Withdrawals

- Positive withdrawal Merkle proof acceptance.
- Invalid sibling order and corrupted leaf rejection.
- Replay rejection after a valid claim.
- Vault release bounce to failed recipient and retry.
- Root bounce authorization for failed vault release messages.
- Jetton/wrapped-gas release authorization and wallet identity checks.
