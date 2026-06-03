struct BatchRootsA {
    prevStateRoot: uint256
    stateRoot: uint256
    txRoot: uint256
}

struct BatchRootsB {
    receiptRoot: uint256
    withdrawalRoot: uint256
    dataHash: uint256
}

struct BatchCommitment {
    rootsA: Cell<BatchRootsA>
    rootsB: Cell<BatchRootsB>
    committedAt: uint32
    finalized: bool
}

struct (0x4c324301) CommitBatch {
    batchNo: uint64
    rootsA: Cell<BatchRootsA>
    rootsB: Cell<BatchRootsB>
}

struct (0x4c324602) FinalizeBatch {
    batchNo: uint64
}

struct (0x4c325704) ClaimWithdrawal {
    batchNo: uint64
    withdrawalId: uint256
    withdrawalLeaf: cell
    merkleProof: cell
}

struct (0x4c324405) DepositTon {
    queryId: uint64
    amount: coins
    l2Recipient: uint256
}

struct (0x4c325206) ReleaseAuthorized {
    withdrawalId: uint256
    assetId: uint32
    recipient: address
    amount: coins
}

struct DepositRecordedExtra {
    l1Sender: address
}

struct (0x4c324407) DepositRecorded {
    queryId: uint64
    depositId: uint256
    assetId: uint32
    amount: coins
    l2Recipient: uint256
    extra: Cell<DepositRecordedExtra>
}

struct (0x4c325208) ReleaseFailed {
    withdrawalId: uint256
    assetId: uint32
    recipient: address
    amount: coins
}

struct (0x7362d09c) JettonTransferNotification {
    queryId: uint64
    amount: coins
    sender: address
    forwardPayload: cell
}

type RootMessage = CommitBatch | FinalizeBatch | ClaimWithdrawal

type BounceableRootMessage = ReleaseAuthorized

type VaultMessage = DepositTon | ReleaseAuthorized | JettonTransferNotification

type BounceableVaultMessage = ReleaseAuthorized
