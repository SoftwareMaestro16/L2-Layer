import "@shared/crypto"
import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

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
