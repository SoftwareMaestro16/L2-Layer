import "@shared/errors"
import "@shared/messages"
import "@shared/proofs"
import "@shared/storage"

contract RollupRoot {
    storage: RollupStorage
    incomingMessages: RootMessage
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy RootMessage.fromSlice(in.body);
    match (msg) {
        CommitBatch => handleCommitBatch(in.senderAddress, msg),
        FinalizeBatch => handleFinalizeBatch(msg),
        ClaimWithdrawal => handleClaimWithdrawal(msg),
        else => {
            assert (in.body.isEmpty()) throw Errors.UnknownOpcode;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy BounceableRootMessage.fromCell(rich.originalBody);
    match (msg) {
        ReleaseAuthorized => {
            var storage = lazy RollupStorage.load();
            storage.claimedWithdrawals.deleteAndGetDeleted(msg.withdrawalId);
            storage.save();
        }
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
    storage.claimedWithdrawals.set(msg.withdrawalId, true);
    storage.save();

    val out = createMessage({
        bounce: BounceMode.RichBounce,
        dest: storage.assetVault,
        value: ton("0.08"),
        body: ReleaseAuthorized {
            withdrawalId: msg.withdrawalId,
            assetId: withdrawal.assetId,
            recipient: withdrawal.recipient,
            amount: withdrawal.amount,
        },
    });
    out.send(SEND_MODE_REGULAR);
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
