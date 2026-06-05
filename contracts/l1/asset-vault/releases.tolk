import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@contracts/asset-vault/jettons"

fun handleReleaseAuthorized(senderAddress: address, msg: ReleaseAuthorized) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.rollupRoot) throw Errors.Unauthorized;

    if (msg.assetId == storage.tonAssetId) {
        attemptTonRelease(storage, msg);
        return;
    }

    val foundAsset = storage.jettonAssets.get(msg.assetId);
    if (foundAsset.isFound) {
        attemptJettonRelease(storage, msg, foundAsset.loadValue().load());
        return;
    }

    recordVaultFailure(storage, msg, RELEASE_FAILURE_UNSUPPORTED_ASSET);
}

fun handleRetryRelease(msg: RetryRelease) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val found = storage.releaseFailures.get(msg.withdrawalId);
    assert (found.isFound) throw Errors.NoFailedWithdrawal;

    val failure = found.loadValue().load();
    if (failure.assetId == storage.tonAssetId) {
        attemptTonRelease(
            storage,
            ReleaseAuthorized {
                withdrawalId: failure.withdrawalId,
                assetId: failure.assetId,
                recipient: failure.recipient,
                amount: failure.amount,
            },
        );
        return;
    }

    val foundAsset = storage.jettonAssets.get(failure.assetId);
    assert (foundAsset.isFound) throw Errors.UnsupportedReleaseAsset;

    attemptJettonRelease(
        storage,
        ReleaseAuthorized {
            withdrawalId: failure.withdrawalId,
            assetId: failure.assetId,
            recipient: failure.recipient,
            amount: failure.amount,
        },
        foundAsset.loadValue().load(),
    );
}

fun handleRecipientReleaseBounce(senderAddress: address, msg: ReleaseAuthorized) {
    var storage = lazy AssetVaultStorage.load();
    assert (senderAddress == msg.recipient) throw Errors.Unauthorized;

    if (msg.assetId == storage.tonAssetId) {
        storage.lockedTon += msg.amount;
    }
    recordVaultFailure(storage, msg, RELEASE_FAILURE_RECIPIENT_BOUNCE);
}

fun attemptTonRelease(storage: AssetVaultStorage, msg: ReleaseAuthorized) {
    assert (msg.amount > 0) throw Errors.BadDepositValue;
    assert (storage.lockedTon >= msg.amount) throw Errors.BadDepositValue;
    storage.lockedTon -= msg.amount;
    storage.releaseFailures.deleteAndGetDeleted(msg.withdrawalId);
    storage.save();

    val out = createMessage({
        bounce: BounceMode.RichBounce,
        dest: msg.recipient,
        value: msg.amount,
        body: msg,
    });
    out.send(SEND_MODE_REGULAR);
}

fun attemptJettonRelease(
    storage: AssetVaultStorage,
    msg: ReleaseAuthorized,
    asset: JettonAssetConfig,
) {
    assert (asset.assetId == msg.assetId) throw Errors.BadAssetConfig;
    assert (msg.amount > 0) throw Errors.BadJettonAmount;

    val queryId = blockchain.logicalTime() as uint64;
    val existing = storage.pendingJettonReleases.get(queryId);
    assert (!existing.isFound) throw Errors.BadJettonQueryId;
    storage.releaseFailures.deleteAndGetDeleted(msg.withdrawalId);
    storage.pendingJettonReleases.set(queryId, msg.toCell());
    storage.save();

    val out = createMessage({
        bounce: BounceMode.RichBounce,
        dest: asset.wallet,
        value: ton("0.08"),
        body: jettonTransferBody(queryId, msg.amount, msg.recipient),
    });
    out.send(SEND_MODE_REGULAR);
}

fun handleJettonTransferBounce(senderAddress: address, queryId: uint64) {
    var storage = lazy AssetVaultStorage.load();
    assertJettonWalletRegistered(storage, senderAddress);

    val found = storage.pendingJettonReleases.get(queryId);
    if (!found.isFound) {
        return;
    }

    val release = found.loadValue().load();
    assertPendingJettonSender(storage, senderAddress, release);
    storage.pendingJettonReleases.deleteAndGetDeleted(queryId);
    recordVaultFailure(storage, release, RELEASE_FAILURE_JETTON_WALLET_BOUNCE);
}

fun recordVaultFailure(storage: AssetVaultStorage, msg: ReleaseAuthorized, reason: uint8) {
    storage
        .releaseFailures
        .set(
            msg.withdrawalId,
            ReleaseFailure {
                withdrawalId: msg.withdrawalId,
                assetId: msg.assetId,
                recipient: msg.recipient,
                amount: msg.amount,
                reason,
                failedAt: blockchain.now(),
                retryCount: nextVaultFailureRetryCount(storage, msg.withdrawalId),
            }.toCell(),
        );
    storage.save();
}

fun nextVaultFailureRetryCount(storage: AssetVaultStorage, withdrawalId: uint256): uint32 {
    val found = storage.releaseFailures.get(withdrawalId);
    if (found.isFound) {
        return found.loadValue().load().retryCount + 1;
    }
    return 0;
}
