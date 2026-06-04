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

fun unlinkedAssetVaultAddress(): address {
    return address("0:0000000000000000000000000000000000000000000000000000000000000000");
}

struct AssetVaultStorage {
    admin: address
    rollupRoot: address
    wrappedGasMinter: address
    paused: bool
    lockedTon: coins
    tonAssetId: uint32
    tonDecimals: uint8
    jettonAssets: map<uint32, Cell<JettonAssetConfig>>
    jettonWalletToAsset: map<uint256, uint32>
    pendingJettonReleases: map<uint64, Cell<ReleaseAuthorized>>
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
    tonAssetId: uint32
    tonDecimals: uint8
    paused: bool
}

struct JettonAssetReply {
    exists: bool
    asset: Cell<JettonAssetConfig>
}
