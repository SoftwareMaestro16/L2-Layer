# L2 Security Audit Roadmap

Date: 2026-06-05

Status: pre-demo audit roadmap for Entropis L2 account, transaction, wallet,
faucet, TVM, and future economics surfaces.

This document defines the audit scope and launch gate for L2-only work. It does
not replace the bridge or L1 contract audits; it complements them with the new
surfaces introduced by EnWallet, faucet OAuth, RAM queues, real TVM execution,
internal messages, and future staking.

## Scope

In scope:

- L2 accounts, account flags, key material references, disabled and system
  states.
- Transaction v2 envelopes, signatures, nonces, expiration, fee assets, memo
  hashes, and receipt semantics.
- Mempool admission, replay protection, per-account pressure, payload limits,
  and reject metrics.
- Executor behavior for deposits, transfers, withdrawals, deploys, contract
  calls, gas accounting, and state-root determinism.
- TVM adapter boundaries, BoC validation, read-only getters, internal messages,
  and replay fixtures.
- Node admin faucet primitives and the planned GitHub OAuth RAM-queue faucet.
- EnWallet browser storage, signing UX, transaction review, and server-side
  faucet proxy boundaries.
- Future staking, reward, commission, delegation, and unbonding state.

Out of scope for this document:

- L1 RollupRoot and AssetVault bytecode audit, except where L2 proofs and
  withdrawals depend on their interfaces.
- Mainnet readiness.
- Production fraud proof implementation.
- Custodial key-management review for operator infrastructure.

## Severity Model

- Critical: direct loss of bridge funds, deterministic state compromise, or
  signing of arbitrary L1 messages from an L2-only path.
- High: repeatable double credit, unauthorized withdrawal, account takeover,
  consensus-breaking nondeterminism, or unauthenticated admin action.
- Medium: testnet feature is usable but needs an explicit gate, mitigation, or
  limitation before public demo.
- Low: documentation, monitoring, or operator ergonomics risk with low direct
  exploitability.

Any new Critical or High finding blocks public demo until fixed or feature-gated
off. Medium findings require mitigation text, monitoring, or a launch limitation.

## Evidence Baseline

Existing evidence to keep current:

- L2 executor and sequencer adversarial tests in
  `crates/l2-core/src/executor_tests.rs`,
  `crates/l2-core/src/executor_tvm_tests.rs`,
  `crates/l2-core/src/sequencer_tests.rs`, and
  `crates/l2-core/src/sequencer_internal_tests.rs`.
- Node API, mempool, storage, and faucet tests in `crates/l2-node/src`.
- SDK transaction, EnWallet, and generated-wrapper vector tests under `sdk`.
- Existing audit notes:
  `docs/security-audit-l2-core-2026-06-05.md` and
  `docs/security-audit-testnet-prototype-2026-06-04.md`.
- Operator diagnostics and safe-error expectations in
  `docs/operator-runbooks.md`.

## Current Finding Summary

No new Critical or High issue is recorded by this roadmap pass. This status is a
launch gate condition, not a permanent guarantee. If later implementation work
finds a Critical or High issue, add a failing adversarial test first, fix the
root cause, and update this document with evidence.

Medium findings:

| ID | Finding | Required mitigation |
| --- | --- | --- |
| M-1 | Browser wallet seed storage and EnWallet UX are not production-safe until encrypted storage, lock/unlock, backup confirmation, and transaction review are complete. | Keep public wallet labeled testnet-only, never log seed material, and block public release of wallet signing until encrypted IndexedDB/WebCrypto storage is implemented. |
| M-2 | Faucet OAuth/RAM backend is an abuse-control boundary but is intentionally non-durable in v1. Restart loses queue, sessions, and cooldown state. | Keep faucet v1 testnet-only, enforce rate limits before public exposure, keep admin token server-side, and document restart behavior. |
| M-3 | Real TVM support depends on deterministic emulator configuration, bounded host boundary, and replay fixtures. | Keep unsupported TVM features fail-closed, require deterministic replay tests before arbitrary public contract deployment, and pin emulator/library configuration. |
| M-4 | Staking and commission logic is not implemented yet. | Keep staking endpoints disabled until deterministic Rust accounting exists and is covered by state-machine tests. |

Low and accepted MVP risks:

- Trusted sequencer can censor or delay user transactions until a forced
  inclusion or proposer mechanism exists.
- Public DA MVP may prove retrievability through a gateway or filesystem mirror,
  not censorship-resistant availability.
- Admin/operator APIs rely on bearer auth and reverse-proxy isolation in testnet.

## Audit Matrix

| Surface | Threat | Existing or planned control | Required tests or gate |
| --- | --- | --- | --- |
| Accounts | Account spoofing or user-to-contract overwrite | Account flags, active public key, reserved zero address rejection, deploy overwrite checks | Account lifecycle tests and deploy-overwrite adversarial tests |
| Transactions | Nonce replay, wrong chain, expired tx, bad signature | Tx v2 domain separation, nonce checks, `valid_until_block`, signature verification | Mempool/API tests for replay, bad signature, wrong chain, and expiration |
| Fees | Fee asset abuse or overflow | Fee asset validation and gas accounting in executor | Overflow and wrong-fee-asset tests before economics changes |
| Mempool | Flooding by one IP/account or payload class | Queue limits, payload limits, reject counters, planned nonce window | Flood tests by account and payload class; operator metrics assertions |
| Executor | State-root manipulation or malformed action | Canonical encoding, deterministic receipts, bounded gas | Receipt-root and state-root determinism tests |
| DeployContract | Contract overwrite or malformed code/data BoC | Code/data hash validation, BoC validation, account state guards | Malformed BoC and active-account overwrite tests |
| CallContract | Malformed inbound BoC or TVM gas exhaustion | Adapter boundary, gas limits, static fail reasons | Adapter success/failure, gas exhaustion, and unsupported-feature tests |
| Internal messages | Message explosion or non-canonical ordering | FIFO queue, per-block limits, max body size, bounce semantics | Internal-message limit and replay tests |
| Get methods | Getter mutation or unbounded stack | Read-only path, bounded stack input/output, timeout | Getter mutation rejection and stack-size tests |
| Faucet | Abuse by GitHub account or L2 address | OAuth, cooldown config, RAM queue, server-side admin token | OAuth/session, cooldown on/off, queue drain, partial batch failure tests |
| Wallet | Seed leakage or blind signing | Encrypted storage task, lock/unlock, backup confirmation, transaction review | Browser storage tests and Playwright smoke before public release |
| Staking | Double reward, early unbond, commission abuse | Future deterministic Rust module before TVM migration | State-machine, rounding, replay, and receipt-root tests |

## Test-Before-Fix Policy

For any Critical or High finding:

1. Add a minimal failing test or fixture that demonstrates the exploit path.
2. Fix the root cause in the owning module.
3. Add a regression test for the fixed behavior if the original test is too
   narrow.
4. Update this document with severity, impact, evidence, and residual risk.
5. Run the full gate before push.

## Manual Adversarial Checklist

Run before any public testnet demo:

- Submit a replayed signed transaction and verify it is rejected before
  execution.
- Submit an expired transaction and a wrong-chain transaction.
- Submit bad-signature, wrong-public-key, and zero-address transactions.
- Attempt to deploy a contract over an active user account and over an existing
  contract account.
- Submit malformed code, data, call body, and getter stack BoCs.
- Exhaust gas in a contract call and verify stable receipt reason.
- Trigger internal message limits and verify no state corruption.
- Flood mempool by account and by payload class; verify limits and metrics.
- Replay a faucet claim id and a repeated GitHub/account claim with cooldown
  enabled.
- Confirm faucet and admin tokens never appear in browser responses or logs.
- Create, lock, unlock, backup, and sign from wallet UI without plaintext seed
  storage.
- Simulate a wallet seed exposure report and verify operator guidance does not
  ask for seed material.
- Keep staking endpoints disabled until staking state exists; if enabled later,
  test early unbond and double-reward attempts.

## Launch Gate

Required checks for tracked changes that affect the listed surfaces:

- Rust: `cargo fmt --all -- --check` and `cargo test --workspace`.
- SDK: `npm ci`, `npm run typecheck`, and vector tests from `sdk`.
- Ecosystem apps: `npm ci`, `npm run typecheck`, `npm run lint` if present,
  and `npm run build` when practical.
- Tolk or Acton changes: `acton build`, `acton test`, `acton check`, and
  `acton fmt --check` through Linux, WSL, Docker, or CI.
- Every tracked change: `py -3 scripts/ci/secret_scan.py --staged` and
  `py -3 scripts/ci/artifact_guard.py --staged`.

## Operator Incident Requirements

Operator runbooks must include safe response steps for:

- Faucet abuse or backend failure.
- Wallet seed exposure reports.
- TVM emulator failure, missing library, or nondeterministic replay finding.
- Future staking or economics accounting anomaly.

Incident notes must never include admin tokens, GitHub tokens, provider API keys,
database URLs, Redis URLs, mnemonics, wallet seeds, raw signed BoCs, or private
deployment endpoints.
