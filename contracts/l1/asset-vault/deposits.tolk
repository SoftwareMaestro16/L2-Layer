import "@shared/crypto"
import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

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
