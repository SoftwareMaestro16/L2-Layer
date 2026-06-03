import "@shared/crypto"
import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

contract AssetVault {
    storage: AssetVaultStorage
    incomingMessages: VaultMessage
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy VaultMessage.fromSlice(in.body);
    match (msg) {
        DepositTon => handleDepositTon(in.senderAddress, in.valueCoins, msg),
        ReleaseAuthorized => handleReleaseAuthorized(in.senderAddress, msg),
        JettonTransferNotification => handleJettonDeposit(in.senderAddress, msg),
        else => {
            assert (in.body.isEmpty()) throw Errors.UnknownOpcode;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
    in.bouncedBody.skipBouncedPrefix();
    val msg = lazy BounceableVaultMessage.fromSlice(in.bouncedBody);
    match (msg) {
        ReleaseAuthorized => {
            var storage = lazy AssetVaultStorage.load();
            storage
                .releaseFailures
                .set(
                    msg.withdrawalId,
                    ReleaseFailed {
                        withdrawalId: msg.withdrawalId,
                        assetId: msg.assetId,
                        recipient: msg.recipient,
                        amount: msg.amount,
                    }.toCell(),
                );
            storage.save();
        }
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

fun handleJettonDeposit(senderAddress: address, msg: JettonTransferNotification) {
    val storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;

    val l2Recipient: uint256 = msg.forwardPayload.beginParse().loadUint(256);
    val depositId = cellReprHash((senderAddress, msg.queryId, msg.amount, l2Recipient).toCell());
    val log = createExternalLogMessage({
        dest: createAddressNone(),
        body: DepositRecorded {
            queryId: msg.queryId,
            depositId,
            assetId: 2,
            amount: msg.amount,
            l2Recipient,
            extra: DepositRecordedExtra { l1Sender: msg.sender }.toCell(),
        },
    });
    log.send(SEND_MODE_REGULAR);
}

fun handleReleaseAuthorized(senderAddress: address, msg: ReleaseAuthorized) {
    var storage = lazy AssetVaultStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.rollupRoot) throw Errors.Unauthorized;

    if (msg.assetId == 1) {
        assert (storage.lockedTon >= msg.amount) throw Errors.BadDepositValue;
        storage.lockedTon -= msg.amount;
        storage.save();

        val out = createMessage({
            bounce: BounceMode.RichBounce,
            dest: msg.recipient,
            value: msg.amount,
        });
        out.send(SEND_MODE_REGULAR);
        return;
    }

    // Jetton and wrapped-gas release are routed through their minter/wallet contracts in v2.
    storage
        .releaseFailures
        .set(
            msg.withdrawalId,
            ReleaseFailed {
                withdrawalId: msg.withdrawalId,
                assetId: msg.assetId,
                recipient: msg.recipient,
                amount: msg.amount,
            }.toCell(),
        );
    storage.save();
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
