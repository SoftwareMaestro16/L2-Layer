import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

fun handleSetAssetVault(senderAddress: address, msg: SetAssetVault) {
    var storage = lazy RollupStorage.load();
    assert (senderAddress == storage.admin) throw Errors.Unauthorized;
    assert (storage.assetVault == unlinkedAssetVaultAddress()) throw Errors.AlreadyConfigured;
    assert (msg.assetVault != unlinkedAssetVaultAddress()) throw Errors.BadAssetConfig;
    assert (storage.lastCommitted == 0) throw Errors.AlreadyConfigured;
    assert (storage.lastFinalized == 0) throw Errors.AlreadyConfigured;

    storage.assetVault = msg.assetVault;
    storage.save();
}

fun handleCommitBatch(senderAddress: address, msg: CommitBatch) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (storage.assetVault != unlinkedAssetVaultAddress()) throw Errors.BadAssetConfig;
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
    assert (!storage.challengeBlocksFinality(msg.batchNo)) throw Errors.ChallengeOpen;

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
