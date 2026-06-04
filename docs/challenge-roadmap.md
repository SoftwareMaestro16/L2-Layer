# Fraud and Challenge Roadmap

This document defines the first-class challenge model for Entropis as an
optimistic TON L2. It is a design milestone, not an enabled bridge feature.
Current testnet MVP remains trusted-sequencer until challenge verification and
resolution are implemented on L1.

## Current Scope

Implemented today:

- L2 blocks contain `prevStateRoot`, `stateRoot`, `txRoot`, `receiptRoot`,
  `withdrawalRoot`, and `dataHash`.
- Batch data is encoded as canonical consensus bytes and stored through the DA
  pipeline before relayer submission.
- The relayer refuses to submit a batch if DA payload retrieval or hash binding
  fails.
- Rust tests cover deterministic batch roots, consensus golden vectors,
  sequencer replay-like flows, and DA missing/corruption failures.

Not implemented today:

- On-chain fraud proof verification in Tolk.
- Sequencer bonds and slashing.
- Forced inclusion for censored transactions.
- Real TVM step proofs for `CallContract`.
- Permissionless challenger service.

## Roles

Sequencer:
Produces ordered L2 blocks, publishes batch payloads, and commits roots to
`RollupRoot`.

Observer:
Downloads batch commitments and DA payloads, replays blocks locally, and reports
root mismatches. An observer does not need signing authority.

Challenger:
An observer that can post an L1 challenge with bond, evidence metadata, and later
resolution data. A challenger must be able to retrieve enough DA and state witness
data to reproduce the disputed transition.

RollupRoot:
Stores commitments, challenge status, finality gates, and future challenge
messages. A challenged batch must not finalize until resolved.

## Data Required For Replay

A challenger node needs:

- Ordered L2 block headers from genesis or from a trusted checkpoint.
- Canonical batch payload bytes for each replayed block.
- Consensus encoding version and hash domains.
- Previous state root or trusted state snapshot root.
- Account leaves and Merkle proofs for accounts touched by the disputed block.
- Deposit events sourced from L1 logs with `l1_tx_hash + lt` replay keys.
- Gas schedule version and deterministic execution config.
- Contract code/data cells for any future `CallContract` execution.
- Internal message queue input and output ordering once async contract execution is
  enabled.
- Receipt, withdrawal, and state root derivation rules.

The MVP can replay transfer/deposit/withdraw blocks from stored block JSON and DA
payloads. Production challengers must not depend on JSON presentation storage;
they must use canonical DA bytes plus Merkle/account witnesses.

## Replay Procedure

1. Read `BatchCommitment(batchNo)` from `RollupRoot`.
2. Retrieve the DA payload by block height or `dataHash`.
3. Verify `hash_domain("l2.batch.data.v1", payload) == dataHash`.
4. Decode canonical transactions and receipts from the payload.
5. Verify transaction order derives the committed `txRoot`.
6. Start from the previous trusted state root.
7. Apply each transaction with the versioned deterministic executor.
8. Derive expected receipt and withdrawal roots.
9. Derive the post-state Merkle root.
10. Compare replayed roots with committed roots.

If `dataHash` cannot be retrieved, the issue is a DA challenge, not a state
transition proof. The batch must remain unfinalized while DA is unavailable.

If replay succeeds but a committed root differs, the challenger locates the first
failing transition by binary search over transaction index or by replaying the
block linearly for smaller batches. The proof target becomes:

- pre-account witnesses,
- transaction bytes,
- deterministic config,
- expected post-account witnesses,
- expected receipt,
- claimed root values.

## Future L1 Message Sketch

These messages are intentionally not implemented yet. Names and fields are the
public interface target for future Tolk work.

```tolk
struct (0x4c324348) ChallengeBatch {
    batchNo: uint64
    challengeKind: uint8  // 1 = missing_da, 2 = invalid_transition
    disputedTxIndex: uint32
    expectedRoot: uint256
    claimedRoot: uint256
    evidenceHash: uint256
}

struct (0x4c325245) RespondChallenge {
    batchNo: uint64
    challengeId: uint256
    responseHash: uint256
    responseData: cell
}

struct (0x4c32524c) ResolveChallenge {
    batchNo: uint64
    challengeId: uint256
    resolutionProof: cell
}

struct (0x4c324649) ForceInclude {
    queueId: uint64
    txHash: uint256
    txDataHash: uint256
}
```

Challenge storage target:

```tolk
struct ChallengeRecord {
    challenger: address
    batchNo: uint64
    challengeKind: uint8
    disputedTxIndex: uint32
    evidenceHash: uint256
    openedAt: uint32
    resolved: bool
}
```

Finalization rule:

- A batch with an open challenge cannot finalize.
- A missing-DA challenge succeeds if payload remains unavailable until the DA
  response deadline.
- An invalid-transition challenge succeeds only with a verifier-supported proof.
- A failed challenge burns or transfers challenger bond to prevent griefing.

## Adversarial Scenarios

Invalid state root:
Challenger replays DA payload from the previous state root and proves the
committed `stateRoot` is not the executor output.

Malformed receipt or withdrawal root:
Challenger recomputes receipt and withdrawal Merkle roots from deterministic
execution and challenges the mismatched root.

Missing DA:
Challenger opens a missing-DA challenge. The sequencer must respond with payload
or a backend-specific availability proof before the response deadline.

Sequencer censorship:
Future forced inclusion queue lets users submit transaction hashes/data
commitments to L1. If the sequencer ignores them past an inclusion deadline, a
challenger can block finalization or force a system transaction.

Malicious sequencer commits:
`CommitBatch` ordering, `prevStateRoot`, batch number, and `dataHash` checks
already limit malformed commitments. Challenge bonds and delayed finality handle
validly formatted but invalidly executed commitments.

Griefing challenges:
Challenges require a bond, one active challenge per `(batchNo, challengeKind,
disputedTxIndex)` key, bounded deadlines, and evidence hashes. Repeated failed
challenges must become expensive.

## Scalability Model

Challenger nodes should be modular:

- DA retriever: fetches canonical batch bytes from Postgres, TON Storage, or a
  future external DA provider.
- Replay engine: deterministic Rust executor plus TVM adapter once enabled.
- Witness provider: supplies touched account proofs and state snapshots.
- Proof generator: builds the smallest L1-verifiable proof for the failing step.
- L1 challenger client: posts and resolves challenges through `RollupRoot`.

Simple observers should not need a full archival node for every audit. They can
start from trusted snapshots and replay bounded block ranges. Full challengers
need enough history or snapshots to construct witnesses.

## Test Roadmap

Existing regression tests that support this roadmap:

- Consensus golden vectors for transaction, receipt, account, withdrawal, and
  block header hashes.
- Deterministic batch builder tests.
- Sequencer block-flow tests.
- DA roundtrip, missing payload, corrupted payload, and replayed payload tests.
- Relayer tests proving missing/corrupted DA prevents L1 submission.

Next Rust tests before enabling L1 challenges:

- Golden replay of a multi-block fixture from canonical DA bytes.
- Corrupted `stateRoot` fixture that identifies the first invalid block.
- Corrupted receipt root fixture that preserves state but fails receipts.
- Missing DA challenge fixture.
- Replay from trusted snapshot plus bounded block range.
- `CallContract` replay fixture after the real TVM adapter is wired.

## MVP Limitations

- Challenge messages are not deployed in Tolk.
- Withdrawals rely on delayed finalization but not fraud proof enforcement.
- DA retrievability is checked by the relayer backend, not by public TON Storage.
- `CallContract` remains fail-closed, so fraud proofs for TVM execution are
  blocked on the real deterministic TVM adapter.
- No sequencer bond exists yet, so slashing is a future design item.

## References

- TON TVM overview: deterministic execution, gas exhaustion, cell-based state.
- TON Merkle proofs: compact verification of cell trees by root hash.
- TON Storage: future external DA backend candidate for batch payload retrieval.
