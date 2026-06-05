# L2 Core Security Audit Pack - 2026-06-05

This pass covers Entropis L2 core, node API, mempool, DA replay surface, SDK
public/admin split, and local contract sandbox readiness. L1 Tolk contracts,
TON provider behavior, signer custody, and live testnet operational keys are out
of scope except where L2 commitments depend on their outputs.

## Scope

- L2 transaction envelope, signatures, nonce, expiration, and fee asset checks.
- Deterministic fee accounting for sequencer, operator, and treasury accounts.
- Account lifecycle, contract deploy, contract calls, BoC validation, and TVM
  adapter boundaries.
- Sequencer batch construction, state roots, receipt roots, withdrawal roots,
  canonical DA bytes, and observer replay.
- Mempool admission, duplicate tx handling, per-account limits, payload limits,
  and rate-limit reporting.
- Public SDK/browser entrypoints and admin-only SDK/operator entrypoints.

## Findings

### High

No open high severity issue remains in this L2-only scope.

### Medium

Prototype TVM execution remains bounded by adapter availability.

- Real TON TVM execution depends on the configured `tonlibjson` boundary.
- Unsupported or unavailable emulator paths fail closed with stable reasons.
- Arbitrary Tolk contract support should stay marked prototype until emulator
  availability and storage proof plans are hardened further.

Browser wallet persistence is intentionally not prescribed.

- SDK/browser creates/imports EnWallet mnemonic material locally.
- The SDK returns mnemonic/keypair to the caller but does not choose storage.
- Production EnWallet UI must use a deliberate secure storage policy and must
  never send seed/private key material to the L2 API.

### Low

The MVP commits events through `receipt_root` and DA, not a separate
`event_root`.

- This is consensus-safe for current L1 schema because receipt leaves include
  typed events.
- A separate event root requires a future block header and RollupRoot schema
  upgrade.

L2 economics Phase A is accounting only.

- Gas and rejection fees are credited to configured L2 accounts and emitted as
  `fee_distributed` events.
- Staking, delegation, unbonding, funded rewards, and reward claims have a
  deterministic Rust state-machine groundwork. Public staking transaction types,
  proposer selection, slashing, and TVM migration are not active consensus rules
  in this pass.

## Attack Matrix

| Area | Result | Evidence |
| --- | --- | --- |
| Nonce replay | Covered | `wrong_nonce_is_rejected`, duplicate tx hash tests, pending nonce window tests |
| Signature spoofing | Covered | sender public-key binding, stale rotated key rejection, bad signature mempool tests |
| Contract deploy overwrite | Covered | deploy overwrite and claimed-user-account rejection tests |
| Malformed CallContract BoC | Covered | malformed and oversized call-body tests reject before adapter |
| Gas griefing | Covered | gas limit, block gas limit, rejection gas, and fee asset tests |
| Fee diversion | Covered | fee bps validation, zero-destination rejection, split/rounding tests, receipt events |
| State root manipulation | Covered | deterministic batch build and observer tampered-root tests |
| Internal message explosion | Covered | adapter message limit and queue-capacity rejection tests |
| Mempool flood | Covered | global/per-account queue, rate limit, bad signature spam, payload-class limits |
| Withdrawal double creation | Covered | duplicate withdraw tx creates one withdrawal leaf |
| DA/block mismatch | Covered | corrupted, missing, wrong-block, and observer replay tests |

## New Tests In This Pass

- `duplicate_withdraw_transaction_creates_only_one_withdrawal_leaf`
- `oversized_receipt_event_list_is_rejected_before_block_root`
- SDK browser/admin split test: browser client has no admin faucet/deposit/block
  helpers and can create an EnWallet mnemonic account.
- Fee accounting tests cover deterministic split, rejection-fee distribution,
  invalid basis points, overflow, zero destinations, and SDK/Rust fee event
  encoding.

## Residual Risks

- Trusted sequencer censorship remains an MVP limitation until L1 challenge and
  forced-inclusion flows are implemented.
- Real TVM execution needs continued deterministic replay coverage with official
  emulator fixtures for non-sample contracts.
- Browser wallet UX must add explicit seed backup, encryption, lock/unlock, and
  clear testnet-only warnings before public wallet distribution.
- Fee accounting currently has configured accounts and a local staking state
  machine, but no public staking/slashing authorization model; operator
  commission and treasury policy must remain explicit deployment config until
  staking transaction types are implemented.

## Launch Gate

L2-only public prototype can proceed if these checks are green:

```text
cargo fmt --all -- --check
cargo test --workspace
npm --prefix sdk run typecheck
npm --prefix sdk run test:vectors
py -3 scripts\ci\secret_scan.py --staged
py -3 scripts\ci\artifact_guard.py --staged
```
