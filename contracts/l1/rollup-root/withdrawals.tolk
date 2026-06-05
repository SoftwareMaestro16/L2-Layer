import "@shared/errors"
import "@shared/messages"
import "@shared/proofs"
import "@shared/storage"

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
