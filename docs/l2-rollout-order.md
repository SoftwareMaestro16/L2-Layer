# Entropis L2 Rollout Order

This document defines the tracked implementation order for moving Entropis from
local L2 development toward a public TON testnet ecosystem. It is not a mainnet
readiness plan and does not contain secrets, wallet material, provider keys,
database URLs, Redis URLs, signer tokens, raw signed BoCs, or private deployment
endpoints.

## Rules

- Work in the order below unless a blocking bug requires a security fix first.
- Each task starts with a branch suggestion, lands as a Conventional Commit, and
  is pushed directly to GitHub after validation. Do not open pull requests for
  these roadmap tasks unless the operator explicitly changes that rule.
- `PLAN.md` is local-only and ignored. Public operator knowledge belongs in
  tracked docs under `docs/`.
- Features that change TON, Tolk, Acton, bridge, TVM, DA, sequencer, or security
  assumptions must update `docs/TON_L2_SKILLS.md`.
- Public docs must say "testnet" when they describe the prototype and must not
  imply mainnet readiness.

## Validation Baseline

Run the checks that match the files changed by the task:

| Change type | Required checks |
| --- | --- |
| Rust/node | `cargo fmt --all -- --check`, `cargo test --workspace` |
| SDK | `npm ci`, `npm run typecheck`, `npm run test:vectors` from `sdk` |
| Ecosystem app | `npm ci`, `npm run typecheck`, `npm run lint` if present, `npm run build` when practical |
| Tolk/Acton | `acton build`, `acton test`, `acton check`, `acton fmt --check` through Linux, WSL, Docker, or CI |
| Any tracked change | `py -3 scripts/ci/secret_scan.py --staged`, `py -3 scripts/ci/artifact_guard.py --staged` |

Before every commit, run `git status --short` and
`git diff --cached --name-only` to verify that local roadmaps, env files,
wallets, keys, caches, databases, generated build output, and raw BoCs are not
staged.

## Rollout Matrix

| Order | Task | Goal | Branch | Commit | Required evidence |
| --- | --- | --- | --- | --- | --- |
| 1 | Root roadmap cleanup | Keep local roadmap files out of Git and keep the tracked docs source clear. | `docs/root-l2-roadmap-plan` | `docs(l2): add root roadmap plan` | Ignored local roadmap status and staged guards. |
| 2 | L2 account and transaction security audit inventory | Harden account lifecycle, tx v2 envelope, replay, nonce, fee asset, mempool, and deterministic receipt assumptions before wallet growth. | `test/l2-account-transaction-hardening` | `test(l2): harden account and transaction security` | Rust adversarial tests, SDK vectors when examples change, audit notes with no Critical/High open issues. |
| 3 | Faucet backend with GitHub OAuth and RAM queue | Add a server-side ecosystem faucet that keeps GitHub/admin tokens out of browsers and batches claims. | `feat/ecosystem-github-faucet-backend` | `feat(ecosystem): add github ent faucet backend` | OAuth/session tests, queue/cooldown tests, mock node API tests, ecosystem typecheck/build. |
| 4 | Node batch faucet primitive | Add storage-backed `POST /v1/admin/faucet/ent/batch` with claim-id idempotency and safe explorer visibility. | `feat/node-batch-ent-faucet` | `feat(node): add batch ent faucet endpoint` | Rust API/storage tests for auth, duplicate claim, duplicate account, invalid account, amount bounds, partial failure, and safe errors. |
| 5 | EnWallet UI security upgrade | Remove plaintext seed storage and add lock/unlock, encrypted storage, backup confirmation, and transaction review. | `feat/wallet-secure-enwallet-storage` | `feat(wallet): secure enwallet storage` | Wallet typecheck/build, browser smoke for create/import/lock/unlock/faucet/transfer, no seed leakage in logs or bundle. |
| 6 | EnWatcher explorer/operator UI | Add dense public explorer and authenticated operator dashboard for accounts, txs, contracts, deposits, withdrawals, DA, faucet, relay, and finality. | `feat/ecosystem-enwatcher-explorer` | `feat(ecosystem): add enwatcher explorer` | Typecheck/lint/build, mock API tests, browser smoke across desktop/mobile, no admin token in public bundle. |
| 7 | EnWallet V5 compatibility/deploy/call | Prove the SDK generated compiled artifact can deploy an EnWallet V5-compatible account through `DeployContract`, call it through `CallContract`, and read metadata/getters without vendoring wallet source in this repo. | `feat/enwallet-v5-l2-flow` | `feat(wallet): verify enwallet v5 l2 flow` | SDK EnWallet vectors, Rust deploy/call/getter tests, wallet UI integration smoke, and external source provenance notes when the compiled artifact changes. |
| 8 | Full TVM/Tolk hardening | Make arbitrary small Acton-built Tolk contracts deterministic, bounded, replayable, and fail-closed on unsupported TVM features. | `feat/l2-full-tvm-hardening` | `feat(l2): harden tvm contract execution` | TVM adapter tests, deterministic replay tests, internal-message limit tests, Acton sample contract checks, SDK deploy/call/getter vectors. |
| 9 | Staking and commission Phase A | Add deterministic fee accounting and visible operator/treasury/sequencer fee destinations without touching L1 bridge custody. | `feat/l2-economics-fee-accounting` | `feat(l2): add deterministic fee accounting` | Fee split tests, overflow/rounding tests, receipt-root stability tests, operator metrics/API tests. |
| 10 | Staking system module or TVM contract Phase B/C | Add stake, delegate, undelegate, unbond, reward, commission, and later TVM migration only after Phase A and TVM hardening are stable. | `feat/l2-staking-module` | `feat(l2): add deterministic staking module` | Staking state-machine tests, early-withdrawal rejection, double-reward tests, deterministic replay, migration equivalence tests before TVM contract migration. |

## Release Gates

A task is complete only when:

- Its acceptance criteria and security checks are documented or tested.
- New public/admin API boundaries have auth tests where applicable.
- Any live testnet work has a manual rehearsal note that excludes secrets.
- Any blocked external dependency is recorded as blocked, not green.
- The branch is pushed after validation with no pull request opened.

## Stop Conditions

Pause rollout and fix the issue first if any of these occur:

- A Critical or High security issue is found.
- A state-root, receipt-root, DA hash, or withdrawal proof changes without an
  intentional consensus migration.
- A browser bundle or public response contains an admin token, provider key,
  seed, mnemonic, raw signed BoC, DB URL, Redis URL, or signer token.
- A public doc or script points to TON mainnet for this prototype.
- Acton/Tolk wrappers are hand-edited instead of regenerated.

## Public Testnet Position

The public target for this rollout is a TON testnet prototype where a user can:

1. Create or import an EnWallet account.
2. Receive test ENT through the faucet path.
3. Submit a signed L2 transfer.
4. Deploy and call a small Tolk contract.
5. Inspect account, tx, contract, deposit, batch, finality, and withdrawal state.

The prototype remains trusted-sequencer and testnet-only until fraud proofs,
production DA, signer operations, wallet security, and staking economics have
separate audited launch gates.
