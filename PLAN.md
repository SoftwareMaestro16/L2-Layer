# Entropis L2 Core, Wallet, Faucet, TVM, Security Roadmap

This is the tracked root roadmap for Entropis L2 development. It replaces local
`STEP*.md` roadmap files and is safe to commit. Do not put mnemonics, private
keys, signer tokens, provider keys, database URLs, Redis URLs, wallet exports,
raw signed BoCs, or private deployment endpoints in this file.

## Current State

- `l2-core` has transaction v2 fields, fee-asset validation, deterministic
  deposits, transfers, withdrawals, `DeployContract`, `CallContract`, contract
  code/data BoC handling, read-only getter boundaries, internal-message support,
  and TVM adapter interfaces.
- `l2-node` has config validation, Postgres/Redis storage, mempool admission,
  explorer APIs, DA, observer replay, faucet primitive, contract state/get-method
  APIs, relayer/finalizer surfaces, readiness, metrics, and operator failures.
- L1 bridge contracts in Tolk cover rollup commitments, finalization, deposits,
  withdrawal claims, TON/Jetton custody surfaces, retries, bounces, and getters.
- `sdk` has browser/admin entrypoints, transaction builders, EnWallet helpers,
  generated wrappers, TON payload helpers, and vector tests.
- `examples/l2-wallet-v5` contains the canonical EnWallet V5 R1 Tolk example.
- `ecosystem/wallet` is a Next app for EnWallet UX; `ecosystem/faucet` is still
  mock-only and must become a real backend service before public use.

## Global Workflow

- Keep human-authored source files small; split before files become catch-all
  modules. Generated wrappers may exceed the line limit and must not be edited by
  hand.
- For tracked changes, run `git status --short` and `git diff --cached --name-only`
  before commit.
- Do not stage local env files, wallets, mnemonics, signer material, API keys,
  node databases, `target/`, `node_modules/`, `.acton/`, `build/`, generated
  deployment output, or raw signed BoCs.
- Rust changes: `cargo fmt --all -- --check` and `cargo test --workspace`.
- SDK changes: `npm ci`, `npm run typecheck`, and relevant vector tests from
  `sdk`.
- Ecosystem frontend/backend changes: `npm ci`, `npm run typecheck`, `npm run lint`
  if present, and `npm run build` when practical.
- Tolk/Acton changes: run `acton build`, `acton test`, `acton check`, and
  `acton fmt --check` through Linux, WSL, Docker, or CI.
- Every tracked change: `py -3 scripts/ci/secret_scan.py --staged` and
  `py -3 scripts/ci/artifact_guard.py --staged`.
- Push directly to GitHub after successful validation unless explicitly told not
  to push.

## 1. Root Roadmap Cleanup

### Goal

Make `PLAN.md` the root tracked roadmap and remove local `STEP*.md` files.

### Current State

`STEP.md`, `STEP_V2.md`, and `STEP_V3.md` were local ignored roadmap files and
are not tracked. `ecosystem/PLAN.md` exists but is not the root source of truth.

### Implementation

- Delete local `STEP.md`, `STEP_V2.md`, and `STEP_V3.md`.
- Add this root `PLAN.md`.
- Keep `ecosystem/PLAN.md` untouched until a later cleanup task rewrites or
  deprecates it.
- Use `PLAN.md` for future roadmap updates and tracked planning decisions.

### Audit And Security

- Verify the deleted STEP files were ignored-only before deletion.
- Verify `PLAN.md` contains no secrets, private endpoints, wallet material, or
  raw BoCs.
- Verify no tracked `.gitignore` change is required for this roadmap cleanup.

### Tests And Checks

- `git status --short --ignored STEP.md STEP_V2.md STEP_V3.md`.
- `git diff --cached --name-only`.
- `py -3 scripts/ci/secret_scan.py --staged`.
- `py -3 scripts/ci/artifact_guard.py --staged`.

### Acceptance Criteria

- Root `PLAN.md` exists and is tracked.
- Local `STEP*.md` files are gone.
- No local roadmap secrets are staged.

### Git

- Branch suggestion: `docs/root-l2-roadmap-plan`.
- Commit: `docs(l2): add root roadmap plan`.

## 2. L2 Protocol And Account Security

### Goal

Harden L2 accounts, transaction envelopes, mempool policy, and state-root
determinism before wider wallet and contract usage.

### Current State

Transactions already include versioning, domain separation, `valid_until_block`,
`fee_asset_id`, memo hash support, and typed transaction kinds. Accounts support
balances, nonce, code/data/storage hashes, code/data BoCs, disabled/system flags,
active public key, and recovery lock surfaces. Mempool admission has replay,
queue, payload, gas, and fee checks.

### Implementation

- Add an account lifecycle audit and follow-up fixes for `user`, `contract`,
  `system`, and `operator` states.
- Make SDK/API examples always emit tx v2 fields explicitly.
- Add public documentation for nonce, expiration, fee asset, rejected receipts,
  and state-root determinism.
- Add per-IP public API throttling in `l2-node` or a documented reverse-proxy
  layer.
- Extend operator metrics with per-class mempool rejection counters for transfer,
  withdraw, deploy, call, and internal-message payloads.
- Add account nonce-window policy so many future nonces cannot flood one account.

### Audit And Security

- Check account spoofing, public-key mismatch, key rotation abuse, recovery-lock
  bypass, reserved zero address, contract overwrite, transaction replay,
  expired transaction inclusion, wrong fee asset, state-root manipulation, and
  malformed payload classes.

### Tests And Checks

- Rust tests for account lifecycle transitions and overwrite rejection.
- Rust tests for tx v2 required fields, expiration, fee asset, bad signature,
  nonce replay, and mempool flooding.
- SDK vector tests for updated explicit tx v2 examples.
- `cargo test --workspace` and SDK typecheck when SDK examples change.

### Acceptance Criteria

- User accounts cannot be silently converted into contracts.
- Contract deployment cannot overwrite active accounts or existing contracts.
- Replayed, expired, wrong-chain, wrong-fee-asset, and bad-signature transactions
  fail before deterministic execution.
- Operator metrics show safe rejection categories.

### Git

- Branch suggestion: `test/l2-account-transaction-hardening`.
- Commit: `test(l2): harden account and transaction security`.

## 3. Staking, Commissions, And L2 Economics

### Goal

Introduce deterministic L2 economics for gas fees, sequencer rewards, staking,
delegation, and commissions without touching L1 bridge custody contracts first.

### Current State

The executor charges gas in ENT asset id `0`, but fees are burned or debited
without a complete sequencer/operator/treasury distribution model. No staking
module exists yet.

### Implementation

- Phase A: add protocol fee accounting in Rust.
  - `operator_commission_bps`.
  - `treasury_fee_bps`.
  - `sequencer_reward_account`.
  - Fee distribution receipts and metrics.
- Phase B: add deterministic Rust staking module.
  - `minimum_stake_ent`.
  - `unbonding_period_blocks`.
  - stake, delegate, undelegate, withdraw-unbonded, slash placeholder, reward
    accrual, commission accounting.
- Phase C: migrate staking/commission logic to TVM contracts after real TVM
  execution is stable and audited.
- Add `GET /v1/staking/status` only after Phase A/B state is implemented.

### Audit And Security

- Check fee overflow, unauthorized reward mint, double reward, commission
  rounding, stake underflow, withdrawal before unbonding, replayed staking tx,
  sequencer self-reward abuse, and censorship visibility.

### Tests And Checks

- Deterministic fee-split tests.
- Staking state machine tests.
- Rounding and overflow adversarial tests.
- Receipt-root stability tests for economics events.
- Future TVM migration tests against equivalent Rust behavior.

### Acceptance Criteria

- Every gas fee has an auditable destination.
- Rewards and commissions are deterministic.
- Stake cannot be withdrawn before unbonding.
- Staking stays L2-only until smart-contract migration is safe.

### Git

- Branch suggestion: `feat/l2-economics-fee-accounting`.
- Commit: `feat(l2): add deterministic fee accounting`.

## 4. Faucet Backend With GitHub OAuth And RAM Batch Queue

### Goal

Replace the mock-only public faucet with a real ecosystem backend that batches
GitHub-authenticated claims and credits 100 ENT on L2 through the node admin
primitive.

### Current State

`l2-node` has an admin-only faucet primitive. `ecosystem/faucet` is mock-only and
does not call the node. `ecosystem/wallet` has a server-side faucet proxy but no
GitHub OAuth, no queue, and no batch worker.

### Implementation

- Build the first real faucet backend in `ecosystem/faucet`.
- Config:
  - `ENTROPIS_API_URL`.
  - `L2_ADMIN_TOKEN`.
  - `GITHUB_CLIENT_ID`.
  - `GITHUB_CLIENT_SECRET`.
  - `FAUCET_AMOUNT_ENT=100`.
  - `FAUCET_BATCH_INTERVAL_MS=10000`.
  - `FAUCET_COOLDOWN_SECONDS=7200`.
  - `FAUCET_ENFORCE_COOLDOWN=false` for current tests.
  - `FAUCET_MAX_BATCH_SIZE=100`.
- GitHub OAuth endpoints:
  - `GET /api/auth/github/start`.
  - `GET /api/auth/github/callback`.
  - `POST /api/auth/logout`.
  - `GET /api/session`.
- Claim endpoints:
  - `POST /api/faucet/claim`.
  - `GET /api/faucet/status`.
  - `GET /api/faucet/batches`.
- Store sessions, claims, cooldowns, pending queue, and batch history in RAM only.
- Batch worker drains pending claims every 10 seconds and calls a node batch
  faucet endpoint when available; until then it may call the single-account
  admin faucet endpoint in a loop behind the same worker boundary.
- Add a planned node endpoint `POST /v1/admin/faucet/ent/batch` with claim-id
  idempotency so repeated test claims are not permanently blocked by account id.
- Keep the future faucet smart contract as a later TVM task, not v1.

### Audit And Security

- Do not expose `L2_ADMIN_TOKEN` or GitHub tokens to the browser.
- Bind claim state to GitHub numeric user id and normalized L2 account id.
- Sanitize logs and API errors.
- Validate raw and friendly Entropis L2 addresses.
- Rate-limit OAuth start, callback, and claim endpoints.
- Enforce same-account and same-GitHub cooldown only when
  `FAUCET_ENFORCE_COOLDOWN=true`.
- Reject reserved zero address.

### Tests And Checks

- Unit tests for config defaults and env parsing.
- OAuth callback tests with mocked GitHub responses.
- Address validation tests.
- Queue drain tests.
- Cooldown disabled/enabled tests.
- Mock node admin API tests for success, failure, retry, and partial batch
  failure.
- `npm ci`, typecheck, lint, build in `ecosystem/faucet`.

### Acceptance Criteria

- A GitHub-authenticated user can submit an L2 account address and enter the RAM
  faucet queue.
- The worker batches claims every 10 seconds.
- Each claim is for 100 ENT.
- Cooldown is implemented for 2 hours but disabled by default in test mode.
- No admin or GitHub token reaches the browser.

### Git

- Branch suggestion: `feat(ecosystem): github-faucet-backend`.
- Commit: `feat(ecosystem): add github ent faucet backend`.

## 5. Node Batch Faucet Primitive

### Goal

Add a storage-backed admin endpoint that accepts multiple faucet claims with
claim-id idempotency.

### Current State

The node faucet primitive grants once per account through
`POST /v1/admin/faucet/ent`. This is safe for a simple demo but does not match a
public RAM-queued faucet service.

### Implementation

- Add `POST /v1/admin/faucet/ent/batch`.
- Request shape:
  - `claims: [{ claim_id, account_id, amount_ent? }]`.
  - `claim_id` is opaque and generated by the faucet backend.
  - `amount_ent` defaults to configured faucet amount and must not exceed a
    configured max.
- Response shape:
  - per-claim status: `granted`, `duplicate_claim`, `duplicate_account`,
    `invalid_account`, `failed`.
  - batch totals and safe static error codes.
- Store claim ids separately from account-level grant state.
- Preserve the existing single-account endpoint.

### Audit And Security

- Admin auth required.
- Reject zero address.
- Reject amount overflow.
- Idempotency must be by `claim_id`.
- No raw backend secrets or bearer tokens in stored errors.
- Partial failures must not double-credit successful claims.

### Tests And Checks

- Rust API tests for auth, zero address, duplicate claim id, duplicate account,
  amount bounds, partial failure behavior, and safe errors.
- Storage tests for claim-id idempotency.
- `cargo fmt --all -- --check`.
- `cargo test --workspace`.

### Acceptance Criteria

- Faucet backend can submit a batch safely.
- Replaying the same claim id is idempotent.
- Existing admin faucet remains compatible.

### Git

- Branch suggestion: `feat(node): batch-ent-faucet`.
- Commit: `feat(node): add batch ent faucet endpoint`.

## 6. EnWallet V5 Production Path

### Goal

Make EnWallet V5 a real Entropis L2 smart-wallet flow: build, deploy, call, read,
and use it from the wallet UI without leaking secrets.

### Current State

`examples/l2-wallet-v5` contains Tolk sources. SDK has EnWallet helpers and
generated wrapper exports. `ecosystem/wallet` can create/import a 24-word seed,
derive keys, sign transfers, read live account data, and request faucet through
a server route. It currently stores seed material in browser localStorage.

### Implementation

- Treat `examples/l2-wallet-v5` as the canonical Tolk contract source.
- Add focused Acton tests for:
  - init storage.
  - external signed request.
  - internal signed request.
  - extension add/remove.
  - signature-disabled mode.
  - invalid seqno, wallet id, signature, valid-until.
  - C5 send-action validation.
- Regenerate wrappers only through Acton when ABI changes.
- Add SDK helper coverage for EnWallet init, signed request body, deploy tx, call
  tx, and getter parsing.
- Connect wallet UI to:
  - deploy EnWallet through `DeployContract`.
  - read EnWallet state through contract state/get-method endpoints.
  - sign transfer/withdraw/call payloads through EnWallet.
- Add encrypted browser storage:
  - WebCrypto-derived key from password.
  - encrypted IndexedDB seed storage.
  - lock/unlock.
  - explicit backup confirmation.
  - session timeout.
- Add transaction review:
  - recipient, amount, asset, fee, nonce, valid-until, raw tx hash.
  - warning on unknown contract calls.

### Audit And Security

- No mnemonic, seed, private key, or raw signed BoC in logs.
- Browser never receives admin token.
- localStorage plaintext seed must be removed before public release.
- Check signature replay, seqno replay, extension abuse, invalid C5 actions,
  action explosion, and malformed cells.

### Tests And Checks

- Acton build/test/check/fmt for EnWallet.
- SDK typecheck and vector tests.
- Wallet typecheck/lint/build.
- Browser smoke for create, backup, lock, unlock, faucet, transfer, and account
  refresh.

### Acceptance Criteria

- EnWallet can be deployed to Entropis L2 with real code/data BoCs.
- EnWallet state is readable.
- UI can create/import/lock/unlock safely.
- Transfers are signed locally and submitted without exposing secrets.

### Git

- Branch suggestion: `feat(wallet): harden enwallet v5 flow`.
- Commit: `feat(wallet): add secure enwallet v5 flow`.

## 7. EnWatcher Explorer And Operator UI

### Goal

Provide a polished public explorer and operator dashboard for L2, bridge, faucet,
contracts, and future economics.

### Current State

`l2-node` exposes explorer APIs and operator endpoints. `ecosystem/README.md`
reserves `explorer` for EnWatcher. No complete explorer/operator UI is tracked.

### Implementation

- Add `ecosystem/explorer` as EnWatcher.
- Public pages:
  - network summary.
  - latest blocks.
  - transaction search.
  - account page.
  - contract page.
  - deposit status.
  - withdrawal status.
  - batch commit/finality.
  - DA payload status.
  - faucet batch status.
- Operator pages:
  - readiness.
  - mempool pressure.
  - relayer/finalizer failures.
  - signer health.
  - faucet queue and batch failures.
  - future staking/economics counters.
- Use existing public/operator APIs only; no direct DB/Redis access.
- Keep admin token server-side or behind an authenticated operator proxy.

### Audit And Security

- No admin token in frontend bundle.
- Escape all user-provided hashes, addresses, reasons, and metadata.
- Public views must not expose internal stack traces or raw provider payloads.
- Operator views require auth.

### Tests And Checks

- Typecheck/lint/build.
- API client tests with mocked responses.
- UI smoke tests across desktop and mobile viewports.
- Secret scan and artifact guard.

### Acceptance Criteria

- User can inspect account, tx, block, contract, deposit, withdrawal, and batch
  state.
- Operator can inspect node/faucet/relayer/finalizer health.
- No secrets are exposed in public UI.

### Git

- Branch suggestion: `feat(ecosystem): add enwatcher explorer`.
- Commit: `feat(ecosystem): add enwatcher explorer`.

## 8. Full TVM/Tolk Smart Contract Support

### Goal

Support arbitrary small Tolk smart contracts on Entropis L2 through deterministic
TVM execution, code/data BoC storage, read-only getters, internal messages, and
replayable state roots.

### Current State

`DeployContract` uses code/data BoCs. `CallContract` routes through an adapter.
Read-only get-method API exists. Sample counter and EnWallet helpers exist. Real
TVM execution depends on a hardened tonlib/TVM emulator boundary and deterministic
config.

### Implementation

- Harden tonlib/TVM emulator loading:
  - explicit library path config.
  - deterministic C7/config.
  - no env/network/filesystem access during execution.
  - stable error code mapping.
- Ensure code/data BoCs are stored and retrieved consistently in memory and
  Postgres.
- Support TVM get-method input stack BoC and output stack BoC safely.
- Implement bounded internal message queue:
  - FIFO ordering.
  - per-block max messages.
  - max body BoC size.
  - bounce/retry semantics.
  - gas limit for internal delivery.
- Enforce limits:
  - code BoC bytes.
  - data BoC bytes.
  - call body bytes.
  - getter stack bytes.
  - action count.
  - internal messages.
- Add sample Tolk contracts:
  - counter.
  - minimal vaultless balance contract.
  - EnWallet V5.
- Add observer replay fixtures for contract execution.

### Audit And Security

- Check nondeterministic fields, malformed BoCs, missing libraries, gas
  exhaustion, storage corruption, internal message explosion, code/data hash
  mismatch, host access, getter mutation, and state-root drift.

### Tests And Checks

- Rust unit tests for emulator adapter success/failure.
- Deterministic replay tests.
- Contract deploy/call/getter E2E tests.
- Acton build/test/check/fmt for sample contracts.
- SDK vector tests for deploy/call/getter payloads.

### Acceptance Criteria

- A small Acton-built Tolk contract deploys, runs, and reads state on L2.
- Same input always yields the same state root and receipt.
- Unsupported or unsafe TVM features fail closed with static reasons.

### Git

- Branch suggestion: `feat(l2): harden full tvm contract support`.
- Commit: `feat(l2): harden tvm contract execution`.

## 9. L2 Security Audit And Test Strategy

### Goal

Run a dedicated L2 audit pass across accounts, transactions, mempool, executor,
TVM, faucet, wallet, and future staking surfaces.

### Current State

There are security docs and adversarial tests, but new surfaces are growing:
EnWallet, faucet OAuth, RAM queue, real TVM, internal messages, and future
economics.

### Implementation

- Add `docs/security-audit-l2-roadmap.md`.
- Audit categories:
  - account spoofing.
  - nonce replay.
  - tx expiration.
  - fee asset abuse.
  - gas griefing.
  - mempool flood.
  - contract overwrite.
  - malformed BoC.
  - TVM nondeterminism.
  - internal message explosion.
  - faucet abuse.
  - GitHub OAuth/session abuse.
  - wallet seed leakage.
  - staking reward manipulation.
- Add tests before fixes for any high-risk finding.
- Add manual adversarial checklist before public testnet demo.

### Audit And Security

- No known critical/high issue may remain open before public demo.
- Medium risks must have explicit mitigation or limitation text.
- Operator runbooks must include incident response steps for faucet, wallet, TVM,
  and staking failures.

### Tests And Checks

- Full Rust workspace.
- SDK typecheck and vectors.
- Ecosystem typecheck/lint/build.
- Acton checks when Tolk changes.
- Secret scan and artifact guard.

### Acceptance Criteria

- Audit doc lists scope, findings, fixes, residual risks, and evidence.
- High severity issues are fixed or feature-gated off.
- Added adversarial tests pass.

### Git

- Branch suggestion: `test(l2): security-audit-roadmap`.
- Commit: `test(l2): audit account wallet faucet and tvm surfaces`.

## 10. Backend/API Additions

### Goal

Add only the API surfaces needed by wallet, faucet, explorer, TVM contracts, and
future staking without leaking operator internals.

### Current State

Existing APIs cover tx submission, accounts, blocks, explorer reads, contract
state/get-method, admin faucet, admin produce block, operator metrics/failures,
batch commits, DA, and proof endpoints.

### Implementation

- Planned APIs:
  - `POST /v1/admin/faucet/ent/batch`.
  - `GET /v1/explorer/faucet/batches` or ecosystem-local equivalent.
  - `GET /v1/explorer/contract/{id}` if existing contract state endpoint is not
    enough for EnWatcher.
  - `GET /v1/staking/status` after staking state exists.
  - `GET /v1/operator/economics` after fee accounting exists.
- Keep admin-only operations behind bearer auth.
- Keep public errors static and safe.
- Ecosystem apps may proxy admin-only operations server-side only.

### Audit And Security

- No admin token in browser.
- No private node config in public responses.
- No raw provider payloads, raw BoCs, or stack traces in errors.
- Auth tests for every operator/admin route.

### Tests And Checks

- Rust API tests for new endpoints.
- SDK/client tests for new DTOs.
- Ecosystem mock API tests.

### Acceptance Criteria

- Faucet, wallet, explorer, contracts, and staking roadmap have clear API
  surfaces.
- Public and admin/operator boundaries are explicit.

### Git

- Branch suggestion: `feat(node): add ecosystem api surfaces`.
- Commit: `feat(node): add ecosystem api surfaces`.

## 11. Documentation And Rollout Order

### Goal

Move in a safe order from roadmap to public testnet ecosystem.

### Implementation Order

1. Root roadmap cleanup.
2. L2 account and transaction security audit inventory.
3. Faucet backend with GitHub OAuth and RAM queue.
4. Node batch faucet primitive.
5. EnWallet UI security upgrade.
6. EnWatcher explorer/operator UI.
7. EnWallet V5 build/test/deploy/call.
8. Full TVM/Tolk hardening.
9. Staking and commission Phase A.
10. Staking system module or TVM contract Phase B/C.

### Acceptance Criteria

- Each task has a branch suggestion, Conventional Commit, tests, security checks,
  and push after validation.
- Features that touch TON, Tolk, Acton, bridge, TVM, DA, sequencer, or security
  update `docs/TON_L2_SKILLS.md`.
- Public docs never imply mainnet readiness.

### Git

- Branch suggestion: `docs(l2): rollout-order`.
- Commit: `docs(l2): document rollout order`.
