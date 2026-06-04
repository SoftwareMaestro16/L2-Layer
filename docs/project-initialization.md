# Project Initialization Baseline

Branch suggestion: `feat/project-baseline`

This document defines the baseline architecture and Git plan for evolving this repository into a production-grade TON L2 optimistic rollup. It is intentionally scoped to architecture, module ownership, tests, security posture, and commit planning; code-moving refactors should happen in follow-up commits with tests.

## 1. Architecture Design

### L1: TON settlement and custody

- `RollupRoot.tolk`: stores batch commitments, challenge-window finality, withdrawal claim replay protection, and authorized release messages to the vault.
- `AssetVault.tolk`: locks TON deposits, accepts Jetton transfer notifications, emits deposit logs for the indexer, releases finalized withdrawals, and records bounced/failed releases.
- Shared Tolk modules own storage schemas, message opcodes, proof validation boundaries, errors, and hashing helpers.

### L2: Rust execution and availability

- Sequencer orders deposits and user transactions, applies deterministic execution, builds block commitments, and prepares L1 batch submissions.
- Mempool validates shape, chain id, signatures, nonces, resource limits, and admission policy before transaction selection.
- Executor applies state transitions with explicit config and without host-dependent side effects.
- StateDB owns account storage, Merkle/cell-compatible state roots, proof generation, and deterministic serialization.
- API exposes REST/WS surfaces while keeping business logic inside injected services.
- Indexer watches TON L1 vault/root activity and converts confirmed events into idempotent L2 inputs.

### Bridge

- Deposit path: TON wallet or Jetton wallet sends L1 message to `AssetVault`; vault emits `DepositRecorded`; indexer ingests canonical event; sequencer credits L2.
- Withdrawal path: user creates L2 withdrawal; sequencer includes withdrawal leaf; `RollupRoot` finalizes batch after challenge window; user submits Merkle proof; `AssetVault` releases funds.
- Registered Jetton releases route through vault-owned Jetton wallets; wrapped-gas release remains future work.

## 2. File/Module Structure

### Current committed implementation

```text
contracts/l1/
  asset-vault/asset_vault.tolk
  rollup-root/rollup_root.tolk
  shared/{crypto,errors,messages,proofs,storage}.tolk

crates/l2-core/src/
  crypto.rs
  executor.rs
  merkle.rs
  sequencer.rs
  state.rs
  types.rs

crates/l2-node/src/main.rs
sdk/src/
  index.ts
  generated/*.gen.ts
tests/contracts.test.tolk
wrappers/*.gen.tolk
```

### Target logical structure

```text
l1/contracts/
  rollup_root/
  asset_vault/
  shared/

l2/
  api/
  config/
  executor/
  indexer/
  mempool/
  sequencer/
  state/

bridge/
  deposit/
  withdrawal/

tests/
  unit/
  integration/
  adversarial/
  determinism/
```

### Migration rule

Do not move everything in one commit. Split the migration by responsibility:

- `refactor(l1-contracts)`: move Tolk contracts and update `Acton.toml` import mappings.
- `refactor(l2-core)`: split `sequencer.rs` into `sequencer`, `mempool`, and `batch_builder`.
- `refactor(l2-api)`: split `main.rs` into router, handlers, app state, producer, and config.
- `feat(bridge-indexer)`: add TON event ingestion interfaces before real network code.

## 3. Code Changes

This initialization change is documentation and governance only:

- Update `AGENTS.md` with Conventional Commit, branch, quality, security, and test gates.
- Add this baseline document to define architecture, module structure, security review, and commit plan.

No runtime code is changed in this step.

## 4. Test Suite

### Current checks

- Rust unit/integration-style tests in `crates/l2-core` cover deposit, transfer, withdrawal, duplicate deposit idempotency, bad nonce rejection, and withdrawal Merkle proof verification.
- Tolk tests in `tests/contracts.test.tolk` cover root deploy/status, batch commit/finalization timing, and TON deposit locking.
- SDK typecheck/build validates TypeScript surfaces and generated wrapper exports.

### Required next tests

- Unit: mempool admission limits, batch builder roots, executor fee accounting, config parsing.
- Adversarial: forged withdrawal proofs, malformed TON cell payloads, replayed deposits, duplicate withdrawals, bad signatures, public-key sender mismatch, gas griefing, mempool flooding.
- Integration: L1 deposit event to L2 credit, L2 withdrawal to L1 claim, bounced release recovery.
- Determinism: same ordered input set produces identical block hash, state root, tx root, receipt root, and withdrawal root across repeated runs.

## 5. Security Audit Notes

| Threat | Baseline defense | Current status |
| --- | --- | --- |
| Invalid nonce replay | Sequencer rejects non-current nonces | Implemented in Rust tests |
| Duplicate deposits | Sequencer deduplicates deposit ids | Implemented in Rust tests |
| Forged withdrawal proof | Root checks finalized batch and Merkle proof | Needs adversarial Tolk tests |
| State root manipulation | Batch commits include prev/new roots | Needs batch builder boundary split |
| Sequencer censorship | Challenge/forced inclusion path needed | Not implemented |
| Malformed Jetton payload | Vault parses canonical `Either Cell ^Cell` forward payload | Strict payload-length checks implemented in Acton tests |
| Gas griefing | Config-driven limits required | Partially implemented |
| Mempool flooding | Admission limits and rate policy needed | Not implemented |
| Bounced release | Root/Vault bounce handlers track failures | Needs deeper inter-contract tests |

## 6. Git Commit Plan

Initial project baseline should be split into conventional commits:

1. `docs(architecture): add project initialization baseline`
   - Adds architecture, module map, testing plan, threat model, and migration rules.
2. `chore(governance): enforce git and security workflow`
   - Updates repository agent rules for commits, branch suggestions, tests, line limits, and security gates.
3. Next code commit: `refactor(l2-mempool): split mempool admission from sequencer`
   - First runtime refactor because `sequencer.rs` currently owns both sequencing and mempool behavior.

## 7. Risks & Mitigations

- Risk: moving to the target structure too quickly can break Cargo workspace paths and Acton import mappings.
  Mitigation: migrate one boundary at a time and run Rust, SDK, and Acton checks after each move.
- Risk: generated wrappers exceed file-size limits.
  Mitigation: treat them as generated artifacts, not human-owned modules, and regenerate from Acton on ABI changes.
- Risk: current API uses local admin endpoints for deposits and block production.
  Mitigation: keep them dev-only and introduce an indexer/relayer abstraction before production deployment.
- Risk: fraud proofs are absent, so the MVP is trusted-sequencer.
  Mitigation: keep finalization conservative, document trust assumptions, and prioritize challenge design.

## Next Architecture Improvement Direction

The next implementation iteration should split `Mempool` out of `crates/l2-core/src/sequencer.rs`, add admission policy config, and introduce adversarial tests for bad signatures, replayed transactions, chain-id mismatch, and mempool flooding. This is the smallest high-value step toward the requested `/l2/mempool` boundary without destabilizing the full workspace layout.
