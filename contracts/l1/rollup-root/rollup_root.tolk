import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@contracts/rollup-root/challenges"
import "@contracts/rollup-root/commitment"
import "@contracts/rollup-root/withdrawals"

contract RollupRoot {
    author: "Fabolous @SoftwareMaestro"
    version: "0.0.1"
    description: "TON L1 settlement root for optimistic L2 batch commitments, finalization, and withdrawal claims."
    storage: RollupStorage
    incomingMessages: RootMessage
}

/// Reads rollup status for explorers, relayers, and SDKs.
get fun rollupStatus(): RollupStatusReply {
    val storage = lazy RollupStorage.load();
    return {
        sequencer: storage.sequencer,
        assetVault: storage.assetVault,
        challengeWindowSec: storage.challengeWindowSec,
        lastCommitted: storage.lastCommitted,
        lastFinalized: storage.lastFinalized,
        paused: storage.paused,
    };
}

/// Reads one stored batch commitment.
get fun commitment(batchNo: uint64): CommitmentReply {
    val storage = lazy RollupStorage.load();
    val found = storage.commitments.get(batchNo);
    if (found.isFound) {
        return { exists: true, commitment: found.loadValue() };
    }
    return { exists: false, commitment: createEmptyCell() as Cell<BatchCommitment> };
}

/// Reads sequencer bond accounting for testnet challengers and operators.
get fun bondStatus(): BondStatusReply {
    val storage = lazy RollupStorage.load();
    val security = storage.security.load();
    return {
        sequencer: storage.sequencer,
        sequencerBond: security.sequencerBond,
        slashedBond: security.slashedBond,
    };
}

/// Reads a batch challenge, if one exists.
get fun batchChallenge(batchNo: uint64): ChallengeReply {
    val storage = lazy RollupStorage.load();
    val found = storage.security.load().challenges.get(batchNo);
    if (found.isFound) {
        return { exists: true, challenge: found.loadValue() };
    }
    return { exists: false, challenge: createEmptyCell() as Cell<BatchChallenge> };
}

/// Reads a failed root-to-vault withdrawal release for retry operators and explorers.
get fun failedWithdrawal(withdrawalId: uint256): ReleaseFailureReply {
    val storage = lazy RollupStorage.load();
    val found = storage.failedWithdrawals.get(withdrawalId);
    if (found.isFound) {
        return { exists: true, failure: found.loadValue() };
    }
    return { exists: false, failure: createEmptyCell() as Cell<ReleaseFailure> };
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy RootMessage.fromSlice(in.body);
    match (msg) {
        CommitBatch => handleCommitBatch(in.senderAddress, msg),
        SetAssetVault => handleSetAssetVault(in.senderAddress, msg),
        FinalizeBatch => handleFinalizeBatch(msg),
        ClaimWithdrawal => handleClaimWithdrawal(msg),
        RetryWithdrawal => handleRetryWithdrawal(msg),
        StakeSequencerBond => handleStakeSequencerBond(in.senderAddress, in.valueCoins, msg),
        ChallengeBatch => handleChallengeBatch(in.senderAddress, in.valueCoins, msg),
        ResolveChallenge => handleResolveChallenge(in.senderAddress, msg),
        else => {
            assert (in.body.isEmpty()) throw Errors.UnknownOpcode;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy BounceableRootMessage.fromCell(rich.originalBody);
    match (msg) {
        ReleaseAuthorized => handleReleaseAuthorizedBounce(in.senderAddress, msg),
        else => {}
    }
}
