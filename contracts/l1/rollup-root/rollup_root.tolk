import "@shared/errors"
import "@shared/messages"
import "@shared/proofs"
import "@shared/storage"

contract RollupRoot {
    author: "Fabolous @SoftwareMaestro"
    version: "0.0.1"
    description: "TON L1 settlement root for optimistic L2 batch commitments, finalization, and withdrawal claims."
    storage: RollupStorage
    incomingMessages: RootMessage
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy RootMessage.fromSlice(in.body);
    match (msg) {
        CommitBatch => handleCommitBatch(in.senderAddress, msg),
        FinalizeBatch => handleFinalizeBatch(msg),
        ClaimWithdrawal => handleClaimWithdrawal(msg),
        RetryWithdrawal => handleRetryWithdrawal(msg),
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

fun handleCommitBatch(senderAddress: address, msg: CommitBatch) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.sequencer) throw Errors.Unauthorized;
    assert (msg.batchNo == storage.lastCommitted + 1) throw Errors.BadBatchNo;

    if (storage.lastCommitted != 0) {
        val previous = storage.commitments.get(storage.lastCommitted);
        assert (previous.isFound) throw Errors.BatchNotFound;
        assert (
            previous.loadValue().load().rootsA.load().stateRoot == msg.rootsA.load().prevStateRoot
        ) throw Errors.BadPrevRoot;
    }

    val commitment: BatchCommitment = {
        rootsA: msg.rootsA,
        rootsB: msg.rootsB,
        committedAt: blockchain.now(),
        finalized: false,
    };

    storage.commitments.set(msg.batchNo, commitment.toCell());
    storage.lastCommitted = msg.batchNo;
    storage.save();
}

fun handleFinalizeBatch(msg: FinalizeBatch) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val found = storage.commitments.get(msg.batchNo);
    assert (found.isFound) throw Errors.BatchNotFound;

    var commitment = found.loadValue().load();
    assert (!commitment.finalized) throw Errors.AlreadyFinalized;
    assert (
        blockchain.now() >= commitment.committedAt + storage.challengeWindowSec
    ) throw Errors.ChallengeWindow;

    commitment.finalized = true;
    storage.commitments.set(msg.batchNo, commitment.toCell());
    if (msg.batchNo > storage.lastFinalized) {
        storage.lastFinalized = msg.batchNo;
    }
    storage.save();
}

fun handleClaimWithdrawal(msg: ClaimWithdrawal) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val found = storage.commitments.get(msg.batchNo);
    assert (found.isFound) throw Errors.BatchNotFound;
    val commitment = found.loadValue().load();
    assert (commitment.finalized) throw Errors.NotFinalized;

    val claimed = storage.claimedWithdrawals.get(msg.withdrawalId);
    assert (!claimed.isFound) throw Errors.AlreadyClaimed;
    assert (
        verifyWithdrawalProof(
            commitment.rootsB.load().withdrawalRoot,
            msg.withdrawalLeaf,
            msg.merkleProof,
        )
    ) throw Errors.BadWithdrawalProof;

    val withdrawal = lazy ReleaseAuthorized.fromCell(msg.withdrawalLeaf);
    assert (withdrawal.withdrawalId == msg.withdrawalId) throw Errors.BadWithdrawalProof;
    storage.claimedWithdrawals.set(msg.withdrawalId, true);
    storage.save();

    sendReleaseToVault(
        storage.assetVault,
        ReleaseAuthorized {
            withdrawalId: msg.withdrawalId,
            assetId: withdrawal.assetId,
            recipient: withdrawal.recipient,
            amount: withdrawal.amount,
        },
    );
}

fun handleRetryWithdrawal(msg: RetryWithdrawal) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val found = storage.failedWithdrawals.get(msg.withdrawalId);
    assert (found.isFound) throw Errors.NoFailedWithdrawal;

    val failure = found.loadValue().load();
    val claimed = storage.claimedWithdrawals.get(msg.withdrawalId);
    assert (claimed.isFound) throw Errors.BadWithdrawalProof;

    storage.failedWithdrawals.deleteAndGetDeleted(msg.withdrawalId);
    storage.save();

    sendReleaseToVault(
        storage.assetVault,
        ReleaseAuthorized {
            withdrawalId: failure.withdrawalId,
            assetId: failure.assetId,
            recipient: failure.recipient,
            amount: failure.amount,
        },
    );
}

fun handleReleaseAuthorizedBounce(senderAddress: address, msg: ReleaseAuthorized) {
    var storage = lazy RollupStorage.load();
    assert (senderAddress == storage.assetVault) throw Errors.Unauthorized;
    val claimed = storage.claimedWithdrawals.get(msg.withdrawalId);
    assert (claimed.isFound) throw Errors.BadWithdrawalProof;

    storage
        .failedWithdrawals
        .set(
            msg.withdrawalId,
            releaseFailureFrom(
                msg,
                RELEASE_FAILURE_VAULT_BOUNCE,
                nextRootFailureRetryCount(storage, msg.withdrawalId),
            ).toCell(),
        );
    storage.save();
}

fun sendReleaseToVault(assetVault: address, release: ReleaseAuthorized) {
    val out = createMessage({
        bounce: BounceMode.RichBounce,
        dest: assetVault,
        value: ton("0.08"),
        body: release,
    });
    out.send(SEND_MODE_REGULAR);
}

fun releaseFailureFrom(
    release: ReleaseAuthorized,
    reason: uint8,
    retryCount: uint32,
): ReleaseFailure {
    return ReleaseFailure {
        withdrawalId: release.withdrawalId,
        assetId: release.assetId,
        recipient: release.recipient,
        amount: release.amount,
        reason,
        failedAt: blockchain.now(),
        retryCount,
    };
}

fun nextRootFailureRetryCount(storage: RollupStorage, withdrawalId: uint256): uint32 {
    val found = storage.failedWithdrawals.get(withdrawalId);
    if (found.isFound) {
        return found.loadValue().load().retryCount + 1;
    }
    return 0;
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

/// Reads a failed root-to-vault withdrawal release for retry operators and explorers.
get fun failedWithdrawal(withdrawalId: uint256): ReleaseFailureReply {
    val storage = lazy RollupStorage.load();
    val found = storage.failedWithdrawals.get(withdrawalId);
    if (found.isFound) {
        return { exists: true, failure: found.loadValue() };
    }
    return { exists: false, failure: createEmptyCell() as Cell<ReleaseFailure> };
}
