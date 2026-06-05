import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@contracts/asset-vault/deposits"
import "@contracts/asset-vault/jettons"
import "@contracts/asset-vault/releases"

contract AssetVault {
    author: "Fabolous @SoftwareMaestro"
    version: "0.0.1"
    description: "TON L1 custody vault for L2 deposits, locked TON accounting, and authorized withdrawal releases."
    storage: AssetVaultStorage
    incomingMessages: VaultMessage
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
