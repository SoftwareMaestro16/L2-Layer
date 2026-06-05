import "@stdlib/gas-payments"
import "c5-register-validation"
import "errors"
import "messages"
import "storage"

contract EnWalletV5 {
    author: "TON Blockchain"
    version: "0.0.1"
    description: "Entropis L2 Wallet V5 R1-compatible smart-contract wallet."
    storage: Storage
    incomingMessages: AllowedMessageToEnWalletV5
    incomingExternal: AllowedExternalMessageToEnWalletV5
}

const SIZE_SIGNATURE = 512

@inline_ref
fun processExtraActions(extraActions: SnakedExtraActions, isExtension: bool) {
    while (true) {
        val action = lazy ExtraAction.fromSlice(extraActions);
        match (action) {
            AddExtensionExtraAction => {
                val (extensionWorkchain, extensionAddrHash) = action.addr.getWorkchainAndHash();
                val myWorkchain = contract.getAddress().getWorkchain();
                assert (myWorkchain == extensionWorkchain) throw Errors.ExtensionWrongWorkchain;

                var storage = lazy Storage.load();
                val inserted = storage.extensions.addIfNotExists(extensionAddrHash, true);
                assert (inserted) throw Errors.AddExtension;
                storage.save();
            }

            RemoveExtensionExtraAction => {
                val (extensionWorkchain, extensionAddrHash) = action.addr.getWorkchainAndHash();
                val myWorkchain = contract.getAddress().getWorkchain();
                assert (myWorkchain == extensionWorkchain) throw Errors.ExtensionWrongWorkchain;

                var storage = lazy Storage.load();
                val removed = storage.extensions.delete(extensionAddrHash);
                assert (removed) throw Errors.RemoveExtension;
                if (!storage.isSignatureAllowed) {
                    assert (
                        !storage.extensions.isEmpty()
                    ) throw Errors.RemoveLastExtensionWhenSignatureDisabled;
                }
                storage.save();
            }

            SetSignatureAllowedExtraAction => {
                assert (isExtension) throw Errors.OnlyExtensionCanChangeSignatureMode;

                var storage = lazy Storage.load();
                assert (
                    storage.isSignatureAllowed != action.allowSignature
                ) throw Errors.ThisSignatureModeAlreadySet;
                if (!action.allowSignature) {
                    assert (
                        !storage.extensions.isEmpty()
                    ) throw Errors.DisableSignatureWhenExtensionsIsEmpty;
                }
                storage.isSignatureAllowed = action.allowSignature;
                storage.save();
            }

            else => throw Errors.UnsupportedAction,
        }
        if (!extraActions.hasNext()) {
            return;
        }
        extraActions = extraActions.getNext();
    }
}

fun processActions(
    outActions: OutActionsCell?,
    hasExtraActions: bool,
    extraActions: SnakedExtraActions,
    isExternal: bool,
    isExtension: bool,
) {
    if (outActions != null) {
        setTvmRegisterC5(outActions.verifyC5Actions(isExternal));
    }
    if (hasExtraActions) {
        processExtraActions(extraActions, isExtension);
    }
}

type AllowedMessageToEnWalletV5 = ExtensionActionRequest | InternalSignedRequest

type AllowedExternalMessageToEnWalletV5 = ExternalSignedRequest

fun onExternalMessage(inMsgBody: slice) {
    val signature = inMsgBody.getLastBits(SIZE_SIGNATURE);
    val signedSlice = inMsgBody.removeLastBits(SIZE_SIGNATURE);

    val msg = AllowedExternalMessageToEnWalletV5.fromSlice(
        signedSlice,
        { throwIfOpcodeDoesNotMatch: Errors.InvalidMessageOperation },
    );

    var storage = lazy Storage.load();

    assert (
        isSignatureValid(signedSlice.hash(), signature, storage.publicKey)
    ) throw Errors.InvalidSignature;
    assert (
        storage.isSignatureAllowed | storage.extensions.isEmpty()
    ) throw Errors.SignatureDisabled;
    assert (msg.seqno == storage.seqno) throw Errors.InvalidSeqno;
    assert (msg.walletId == storage.subwalletId) throw Errors.InvalidWalletId;
    assert (msg.validUntil > blockchain.now()) throw Errors.Expired;

    acceptExternalMessage();

    storage.isSignatureAllowed = true;
    storage.seqno += 1;
    storage.save();

    commitContractDataAndActions();
    processActions(msg.outActions, msg.hasExtraActions, msg.extraActions, true, false);
}

@on_bounced_policy("manual")
fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessageToEnWalletV5.fromSlice(in.body);

    match (msg) {
        InternalSignedRequest => {
            val bodyLongEnough = in.body.remainingBitsCount() >= SIZE_SIGNATURE + 128;
            if (!bodyLongEnough) {
                return;
            }

            val signature = in.body.getLastBits(SIZE_SIGNATURE);
            val signedSlice = in.body.removeLastBits(SIZE_SIGNATURE);

            var storage = lazy Storage.load();
            if (!isSignatureValid(signedSlice.hash(), signature, storage.publicKey)) {
                return;
            }

            assert (
                storage.isSignatureAllowed | storage.extensions.isEmpty()
            ) throw Errors.SignatureDisabled;
            assert (msg.seqno == storage.seqno) throw Errors.InvalidSeqno;
            assert (msg.walletId == storage.subwalletId) throw Errors.InvalidWalletId;
            assert (msg.validUntil > blockchain.now()) throw Errors.Expired;

            storage.isSignatureAllowed = true;
            storage.seqno += 1;
            storage.save();

            processActions(msg.outActions, msg.hasExtraActions, msg.extraActions, false, false);
        }

        ExtensionActionRequest => {
            val (senderWorkchain, senderAddrHash) = in.senderAddress.getWorkchainAndHash();
            val myWorkchain = contract.getAddress().getWorkchain();
            if (myWorkchain != senderWorkchain) {
                return;
            }

            val storage = lazy Storage.load();
            if (!storage.extensions.exists(senderAddrHash)) {
                return;
            }

            processActions(msg.outActions, msg.hasExtraActions, msg.extraActions, false, true);
        }

        else => {
            // Unknown or empty wallet body is ignored like TON wallet contracts.
        }
    }
}

get fun is_signature_allowed(): bool {
    val storage = lazy Storage.load();
    return storage.isSignatureAllowed;
}

get fun seqno(): int {
    val storage = lazy Storage.load();
    return storage.seqno;
}

get fun get_wallet_id(): int {
    val storage = lazy Storage.load();
    return storage.subwalletId;
}

get fun get_subwallet_id(): int {
    val storage = lazy Storage.load();
    return storage.subwalletId;
}

get fun get_public_key(): int {
    val storage = lazy Storage.load();
    return storage.publicKey;
}

get fun get_extensions(): map<uint256, bool> {
    val storage = lazy Storage.load();
    return storage.extensions;
}
