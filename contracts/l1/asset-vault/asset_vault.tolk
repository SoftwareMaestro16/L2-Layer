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
    if (isJettonTransferNotification(in.body)) {
        handleJettonDepositBody(in.senderAddress, in.body);
        return;
    }

    val msg = lazy VaultMessage.fromSlice(in.body);
    match (msg) {
        DepositTon => handleDepositTon(in.senderAddress, in.valueCoins, msg),
        ReleaseAuthorized => handleReleaseAuthorized(in.senderAddress, msg),
        RetryRelease => handleRetryRelease(msg),
        RegisterJettonAsset => handleRegisterJettonAsset(in.senderAddress, msg),
        JettonTransferNotification => throw Errors.BadForwardPayload,
        JettonExcesses => handleJettonExcesses(in.senderAddress, msg),
        else => {
            assert (in.body.isEmpty()) throw Errors.UnknownOpcode;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    var body = rich.originalBody.beginParse();
    if (!body.isEmpty()) {
        val op = body.loadUint(32);
        if (op == 0x0f8a7ea5) {
            val queryId = body.loadUint(64);
            handleJettonTransferBounce(in.senderAddress, queryId);
            return;
        }
    }

    val msg = lazy BounceableVaultMessage.fromCell(rich.originalBody);
    match (msg) {
        ReleaseAuthorized => handleRecipientReleaseBounce(in.senderAddress, msg),
        else => {}
    }
}

fun handleDepositTon(senderAddress: address, valueCoins: coins, msg: DepositTon) {
    var storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (msg.amount > 0) throw Errors.BadDepositValue;
    assert (msg.l2Recipient != 0) throw Errors.BadDepositValue;
    assert (valueCoins >= msg.amount) throw Errors.BadDepositValue;

    storage.lockedTon += msg.amount;
    storage.save();

    val depositId = cellReprHash(
        (senderAddress, msg.queryId, storage.tonAssetId, msg.amount, msg.l2Recipient).toCell(),
    );
    val log = createExternalLogMessage({
        dest: createAddressNone(),
        body: DepositRecorded {
            queryId: msg.queryId,
            depositId,
            assetId: storage.tonAssetId,
            amount: msg.amount,
            l2Recipient: msg.l2Recipient,
            extra: DepositRecordedExtra { l1Sender: senderAddress }.toCell(),
        },
    });
    log.send(SEND_MODE_REGULAR);
}

fun isJettonTransferNotification(body: slice): bool {
    if (body.remainingBitsCount() < 32) {
        return false;
    }
    return body.preloadUint(32) == 0x7362d09c;
}

fun handleJettonDepositBody(senderAddress: address, body: slice) {
    var s = body;
    assert (s.loadUint(32) == 0x7362d09c) throw Errors.UnknownOpcode;
    val queryId = s.loadUint(64) as uint64;
    val amount = s.loadCoins();
    val sender = s.loadAddress();
    val l2Recipient = decodeL2Recipient(mutate s);
    assert (s.isEmpty()) throw Errors.BadForwardPayload;
    handleJettonDeposit(senderAddress, queryId, amount, sender, l2Recipient);
}

fun handleRegisterJettonAsset(senderAddress: address, msg: RegisterJettonAsset) {
    var storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.admin) throw Errors.Unauthorized;
    assert (msg.assetId != storage.tonAssetId) throw Errors.BadAssetConfig;
    assert (msg.assetId != 0) throw Errors.BadAssetConfig;
    assert (msg.decimals <= 18) throw Errors.BadAssetDecimals;

    val walletKey = jettonWalletKey(msg.wallet);
    val existingWallet = storage.jettonWalletToAsset.get(walletKey);
    if (existingWallet.isFound) {
        assert (existingWallet.loadValue() == msg.assetId) throw Errors.BadJettonWallet;
    }
    val existingAsset = storage.jettonAssets.get(msg.assetId);
    if (existingAsset.isFound) {
        val previous = existingAsset.loadValue().load();
        if (previous.wallet != msg.wallet) {
            storage.jettonWalletToAsset.deleteAndGetDeleted(jettonWalletKey(previous.wallet));
        }
    }

    val asset = JettonAssetConfig {
        assetId: msg.assetId,
        master: msg.master,
        wallet: msg.wallet,
        decimals: msg.decimals,
    };
    storage.jettonAssets.set(msg.assetId, asset.toCell());
    storage.jettonWalletToAsset.set(walletKey, msg.assetId);
    storage.save();
}

fun handleJettonDeposit(
    senderAddress: address,
    queryId: uint64,
    amount: coins,
    sender: address,
    l2Recipient: uint256,
) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (amount > 0) throw Errors.BadJettonAmount;

    val foundAssetId = storage.jettonWalletToAsset.get(jettonWalletKey(senderAddress));
    assert (foundAssetId.isFound) throw Errors.BadJettonWallet;
    val assetId = foundAssetId.loadValue();
    val foundAsset = storage.jettonAssets.get(assetId);
    assert (foundAsset.isFound) throw Errors.BadJettonWallet;
    val asset = foundAsset.loadValue().load();
    assert (asset.wallet == senderAddress) throw Errors.BadJettonWallet;

    val depositId = cellReprHash(
        (
            senderAddress,
            queryId,
            asset.assetId,
            sender,
            amount,
            l2Recipient,
        ).toCell(),
    );
    val log = createExternalLogMessage({
        dest: createAddressNone(),
        body: DepositRecorded {
            queryId,
            depositId,
            assetId: asset.assetId,
            amount,
            l2Recipient,
            extra: DepositRecordedExtra { l1Sender: sender }.toCell(),
        },
    });
    log.send(SEND_MODE_REGULAR);
}

fun handleJettonExcesses(senderAddress: address, msg: JettonExcesses) {
    var storage = lazy AssetVaultStorage.load();
    assertJettonWalletRegistered(storage, senderAddress);
    val found = storage.pendingJettonReleases.get(msg.queryId);
    if (found.isFound) {
        assertPendingJettonSender(storage, senderAddress, found.loadValue().load());
    }
    storage.pendingJettonReleases.deleteAndGetDeleted(msg.queryId);
    storage.save();
}

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

fun jettonTransferBody(queryId: uint64, amount: coins, recipient: address): cell {
    return beginCell()
        .storeUint(0x0f8a7ea5, 32)
        .storeUint(queryId, 64)
        .storeCoins(amount)
        .storeAddress(recipient)
        .storeAddress(contract.getAddress())
        .storeBool(false)
        .storeCoins(0)
        .storeBool(false)
        .endCell();
}

fun decodeL2Recipient(mutate payload: slice): uint256 {
    assert (payload.remainingBitsCount() >= 1) throw Errors.BadForwardPayload;
    val isRef = payload.loadBool();
    if (isRef) {
        val ref = payload.loadRef();
        assert (payload.isEmpty()) throw Errors.BadForwardPayload;
        return decodeL2RecipientCell(ref);
    }
    val recipient = payload.loadUint(256);
    assert (payload.isEmpty()) throw Errors.BadForwardPayload;
    assert (recipient != 0) throw Errors.BadForwardPayload;
    return recipient as uint256;
}

fun decodeL2RecipientCell(payload: cell): uint256 {
    var s = payload.beginParse();
    val recipient = s.loadUint(256);
    assert (s.isEmpty()) throw Errors.BadForwardPayload;
    assert (recipient != 0) throw Errors.BadForwardPayload;
    return recipient as uint256;
}

fun jettonWalletKey(wallet: address): uint256 {
    val (_, hash) = wallet.getWorkchainAndHash();
    return hash;
}

fun assertJettonWalletRegistered(storage: AssetVaultStorage, wallet: address) {
    val foundAssetId = storage.jettonWalletToAsset.get(jettonWalletKey(wallet));
    assert (foundAssetId.isFound) throw Errors.BadJettonWallet;
    val foundAsset = storage.jettonAssets.get(foundAssetId.loadValue());
    assert (foundAsset.isFound) throw Errors.BadJettonWallet;
    val asset = foundAsset.loadValue().load();
    assert (asset.wallet == wallet) throw Errors.BadJettonWallet;
}

fun assertPendingJettonSender(
    storage: AssetVaultStorage,
    wallet: address,
    release: ReleaseAuthorized,
) {
    val foundAsset = storage.jettonAssets.get(release.assetId);
    assert (foundAsset.isFound) throw Errors.BadJettonWallet;
    val asset = foundAsset.loadValue().load();
    assert (asset.wallet == wallet) throw Errors.BadJettonWallet;
}

/// Reads vault status for indexers and operators.
get fun vaultStatus(): VaultStatusReply {
    val storage = lazy AssetVaultStorage.load();
    return {
        rollupRoot: storage.rollupRoot,
        wrappedGasMinter: storage.wrappedGasMinter,
        lockedTon: storage.lockedTon,
        tonAssetId: storage.tonAssetId,
        tonDecimals: storage.tonDecimals,
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

/// Reads configured Jetton assets for bridge operators and indexers.
get fun jettonAsset(assetId: uint32): JettonAssetReply {
    val storage = lazy AssetVaultStorage.load();
    val found = storage.jettonAssets.get(assetId);
    if (found.isFound) {
        return { exists: true, asset: found.loadValue() };
    }
    return { exists: false, asset: createEmptyCell() as Cell<JettonAssetConfig> };
}
