# L2 Core Security Audit Pack - 2026-06-05

This pass covers Entropis L2 core, node API, mempool, DA replay surface, SDK
public/admin split, and local contract sandbox readiness. L1 Tolk contracts,
TON provider behavior, signer custody, and live testnet operational keys are out
of scope except where L2 commitments depend on their outputs.

## Scope

- L2 transaction envelope, signatures, nonce, expiration, and fee asset checks.
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

Mempool admission is intentionally state-agnostic for account key binding.

- The mempool verifies envelope shape, public-key format, signature validity,
  gas/size policy, replay windows, and flood limits.
- It does not reject `from != derive(public_key)` because rotated accounts use
  `active_public_key` stored in state and cannot be validated without the
  sequencer snapshot.
- The sequencer and observer replay enforce account type, disabled/recovery
  flags, and active-public-key binding before deterministic execution.

## Attack Matrix

| Area | Result | Evidence |
| --- | --- | --- |
| Nonce replay | Covered | `wrong_nonce_is_rejected`, duplicate tx hash tests, pending nonce window tests |
| Signature spoofing | Covered | state-aware sender public-key binding, mismatched public key rejection, stale rotated key rejection, bad signature mempool tests |
| Contract deploy overwrite | Covered | deploy overwrite and claimed-user-account rejection tests |
| Malformed CallContract BoC | Covered | malformed and oversized call-body tests reject before adapter |
| Gas griefing | Covered | gas limit, block gas limit, rejection gas, and fee asset tests |
| State root manipulation | Covered | deterministic batch build and observer tampered-root tests |
| Internal message explosion | Covered | adapter message limit and queue-capacity rejection tests |
| Mempool flood | Covered | global/per-account queue, rate limit, bad signature spam, payload-class limits |
| Tx envelope downgrade | Covered | tx version, domain separator, kind version, and fee-asset mempool counters |
| Account lifecycle flags | Covered | disabled, recovery-locked, contract-only, system-only, and operator-account tests |
| Withdrawal double creation | Covered | duplicate withdraw tx creates one withdrawal leaf |
| DA/block mismatch | Covered | corrupted, missing, wrong-block, and observer replay tests |

## New Tests In This Pass

- `duplicate_withdraw_transaction_creates_only_one_withdrawal_leaf`
- `oversized_receipt_event_list_is_rejected_before_block_root`
- `mismatched_public_key_cannot_spoof_sender_account`
- `recovery_locked_account_cannot_rotate_or_transfer`
- `system_account_cannot_submit_public_transactions`
- `operator_account_can_submit_public_transactions`
- `tx_v2_envelope_rejections_are_counted_and_not_enqueued`
- SDK browser/admin split test: browser client has no admin faucet/deposit/block
  helpers and can create an EnWallet mnemonic account.

## Residual Risks

- Trusted sequencer censorship remains an MVP limitation until L1 challenge and
  forced-inclusion flows are implemented.
- Real TVM execution needs continued deterministic replay coverage with official
  emulator fixtures for non-sample contracts.
- State-invalid but well-signed public transactions can occupy bounded mempool
  space until block production because account-key binding is state-aware. Per-IP,
  per-account, nonce-window, queue, and payload caps bound this DoS surface.
- Browser wallet UX must add explicit seed backup, encryption, lock/unlock, and
  clear testnet-only warnings before public wallet distribution.

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
