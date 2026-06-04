import "@shared/crypto"
import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

contract AssetVault {
    author: "Fabolous @SoftwareMaestro"
    version: "0.0.1"
    description: "TON L1 custody vault for L2 deposits, locked TON accounting, and authorized withdrawal releases."
    storage: AssetVaultStorage
    incomingMessages: VaultMessage
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy VaultMessage.fromSlice(in.body);
    match (msg) {
        DepositTon => handleDepositTon(in.senderAddress, in.valueCoins, msg),
        ReleaseAuthorized => handleReleaseAuthorized(in.senderAddress, msg),
        RetryRelease => handleRetryRelease(msg),
        JettonTransferNotification => handleJettonDeposit(in.senderAddress, msg),
        else => {
            assert (in.body.isEmpty()) throw Errors.UnknownOpcode;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy BounceableVaultMessage.fromCell(rich.originalBody);
    match (msg) {
        ReleaseAuthorized => handleRecipientReleaseBounce(in.senderAddress, msg),
        else => {}
    }
}

fun handleDepositTon(senderAddress: address, valueCoins: coins, msg: DepositTon) {
    var storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (valueCoins >= msg.amount) throw Errors.BadDepositValue;

    storage.lockedTon += msg.amount;
    storage.save();

    val depositId = cellReprHash(
        (senderAddress, msg.queryId, msg.amount, msg.l2Recipient).toCell(),
    );
    val log = createExternalLogMessage({
        dest: createAddressNone(),
        body: DepositRecorded {
            queryId: msg.queryId,
            depositId,
            assetId: 1,
            amount: msg.amount,
            l2Recipient: msg.l2Recipient,
            extra: DepositRecordedExtra { l1Sender: senderAddress }.toCell(),
        },
    });
    log.send(SEND_MODE_REGULAR);
}

fun handleJettonDeposit(_senderAddress: address, _msg: JettonTransferNotification): never {
    throw Errors.UnsupportedJettonDeposit;
}

fun handleReleaseAuthorized(senderAddress: address, msg: ReleaseAuthorized) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.rollupRoot) throw Errors.Unauthorized;

    if (msg.assetId == 1) {
        attemptTonRelease(storage, msg);
        return;
    }

    // Jetton and wrapped-gas release are routed through their minter/wallet contracts in v2.
    recordVaultFailure(storage, msg, RELEASE_FAILURE_UNSUPPORTED_ASSET);
}

fun handleRetryRelease(msg: RetryRelease) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val found = storage.releaseFailures.get(msg.withdrawalId);
    assert (found.isFound) throw Errors.NoFailedWithdrawal;

    val failure = found.loadValue().load();
    assert (failure.assetId == 1) throw Errors.UnsupportedReleaseAsset;

    attemptTonRelease(
        storage,
        ReleaseAuthorized {
            withdrawalId: failure.withdrawalId,
            assetId: failure.assetId,
            recipient: failure.recipient,
            amount: failure.amount,
        },
    );
}

fun handleRecipientReleaseBounce(senderAddress: address, msg: ReleaseAuthorized) {
    var storage = lazy AssetVaultStorage.load();
    assert (senderAddress == msg.recipient) throw Errors.Unauthorized;

    if (msg.assetId == 1) {
        storage.lockedTon += msg.amount;
    }
    recordVaultFailure(storage, msg, RELEASE_FAILURE_RECIPIENT_BOUNCE);
}

fun attemptTonRelease(storage: AssetVaultStorage, msg: ReleaseAuthorized) {
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

/// Reads vault status for indexers and operators.
get fun vaultStatus(): VaultStatusReply {
    val storage = lazy AssetVaultStorage.load();
    return {
        rollupRoot: storage.rollupRoot,
        wrappedGasMinter: storage.wrappedGasMinter,
        lockedTon: storage.lockedTon,
        paused: storage.paused,
    };
}

/// Reads a failed vault-to-recipient release for retry operators and explorers.
get fun failedRelease(withdrawalId: uint256): ReleaseFailureReply {
    val storage = lazy AssetVaultStorage.load();
    val found = storage.releaseFailures.get(withdrawalId);
    if (found.isFound) {
        return { exists: true, failure: found.loadValue() };
    }
    return { exists: false, failure: createEmptyCell() as Cell<ReleaseFailure> };
}
