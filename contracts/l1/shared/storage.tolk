import "messages"

struct RollupStorage {
    admin: address
    sequencer: address
    assetVault: address
    challengeWindowSec: uint32
    lastCommitted: uint64
    lastFinalized: uint64
    paused: bool
    security: Cell<RollupSecurityState>
    commitments: map<uint64, Cell<BatchCommitment>>
    claimedWithdrawals: map<uint256, bool>
    failedWithdrawals: map<uint256, Cell<ReleaseFailure>>
}

struct RollupSecurityState {
    sequencerBond: coins
    slashedBond: coins
    challenges: map<uint64, Cell<BatchChallenge>>
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

fun emptyRollupSecurityState(): RollupSecurityState {
    return RollupSecurityState {
        sequencerBond: 0,
        slashedBond: 0,
        challenges: [],
    };
}

fun RollupStorage.challengeBlocksFinality(self, batchNo: uint64): bool {
    val found = self.security.load().challenges.get(batchNo);
    if (found.isFound) {
        return found.loadValue().load().status != CHALLENGE_STATUS_REJECTED;
    }
    return false;
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

struct BondStatusReply {
    sequencer: address
    sequencerBond: coins
    slashedBond: coins
}

struct ChallengeReply {
    exists: bool
    challenge: Cell<BatchChallenge>
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
