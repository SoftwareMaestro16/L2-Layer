import "messages"

struct RollupStorage {
    admin: address
    sequencer: address
    assetVault: address
    challengeWindowSec: uint32
    lastCommitted: uint64
    lastFinalized: uint64
    paused: bool
    commitments: map<uint64, Cell<BatchCommitment>>
    claimedWithdrawals: map<uint256, bool>
    failedWithdrawals: map<uint256, Cell<ReleaseFailure>>
}

fun RollupStorage.load(): RollupStorage {
    return RollupStorage.fromCell(contract.getData());
}

fun RollupStorage.save(self) {
    contract.setData(self.toCell());
}

struct AssetVaultStorage {
    admin: address
    rollupRoot: address
    wrappedGasMinter: address
    paused: bool
    lockedTon: coins
    releaseFailures: map<uint256, Cell<ReleaseFailure>>
}

fun AssetVaultStorage.load(): AssetVaultStorage {
    return AssetVaultStorage.fromCell(contract.getData());
}

fun AssetVaultStorage.save(self) {
    contract.setData(self.toCell());
}

struct RollupStatusReply {
    sequencer: address
    assetVault: address
    challengeWindowSec: uint32
    lastCommitted: uint64
    lastFinalized: uint64
    paused: bool
}

struct CommitmentReply {
    exists: bool
    commitment: Cell<BatchCommitment>
}

struct ReleaseFailureReply {
    exists: bool
    failure: Cell<ReleaseFailure>
}

struct VaultStatusReply {
    rollupRoot: address
    wrappedGasMinter: address
    lockedTon: coins
    paused: bool
}
