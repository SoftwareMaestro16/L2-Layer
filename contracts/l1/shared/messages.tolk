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

struct (0x4c324c09) SetAssetVault {
    assetVault: address
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

struct (0x4c325705) RetryWithdrawal {
    withdrawalId: uint256
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

struct (0x4c325207) RetryRelease {
    withdrawalId: uint256
}

struct (0x4c32450b) RegisterJettonAsset {
    assetId: uint32
    master: address
    wallet: address
    decimals: uint8
}

const RELEASE_FAILURE_VAULT_BOUNCE = 1
const RELEASE_FAILURE_RECIPIENT_BOUNCE = 2
const RELEASE_FAILURE_UNSUPPORTED_ASSET = 3
const RELEASE_FAILURE_JETTON_WALLET_BOUNCE = 4

struct ReleaseFailure {
    withdrawalId: uint256
    assetId: uint32
    recipient: address
    amount: coins
    reason: uint8
    failedAt: uint32
    retryCount: uint32
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
    reason: uint8
}

struct (0x7362d09c) JettonTransferNotification {
    queryId: uint64
    amount: coins
    sender: address
    forwardPayload: cell
}

struct (0xd53276db) JettonExcesses {
    queryId: uint64
}

struct JettonAssetConfig {
    assetId: uint32
    master: address
    wallet: address
    decimals: uint8
}

type RootMessage = CommitBatch | SetAssetVault | FinalizeBatch | ClaimWithdrawal | RetryWithdrawal

type BounceableRootMessage = ReleaseAuthorized

type VaultMessage =
    | DepositTon
    | ReleaseAuthorized
    | RetryRelease
    | RegisterJettonAsset
    | JettonTransferNotification
    | JettonExcesses

type BounceableVaultMessage = ReleaseAuthorized
