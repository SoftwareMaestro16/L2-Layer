struct (0x02) AddExtensionExtraAction {
    addr: address
}

struct (0x03) RemoveExtensionExtraAction {
    addr: address
}

struct (0x04) SetSignatureAllowedExtraAction {
    allowSignature: bool
}

type ExtraAction =
    | AddExtensionExtraAction
    | RemoveExtensionExtraAction
    | SetSignatureAllowedExtraAction

/// "OutActionList" is what the wallet pushes into TVM c5.
type OutActionsCell = cell

/// `action_send_msg` from block.tlb. Wallet V5 accepts only this C5 action family.
struct (0x0ec3c86d) OutActionWithSendMessageOnly {
    prev: Cell<OutActionWithSendMessageOnlyOrEmpty>
    sendMode: uint8
    outMsg: cell
}

type OutActionWithSendMessageOnlyOrEmpty = OutActionWithSendMessageOnly | void

/// Extra actions are represented in snake-cell format.
type SnakedExtraActions = RemainingBitsAndRefs

fun SnakedExtraActions.hasNext(self): bool {
    return self.remainingRefsCount() != 0;
}

fun SnakedExtraActions.getNext(self): SnakedExtraActions {
    return self.preloadRef().beginParse();
}

struct SnakedExtraActionsClientType {
    action: ExtraAction
    next: Cell<SnakedExtraActionsClientType> | void
}

struct (0x6578746E) ExtensionActionRequest {
    queryId: uint64
    @abi.clientType(Cell<OutActionWithSendMessageOnly>?)
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
}

struct (0x73696E74) InternalSignedRequest {
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    @abi.clientType(Cell<OutActionWithSendMessageOnly>?)
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
    // The signed request ends with a 512-bit Ed25519 signature.
}

struct (0x7369676E) ExternalSignedRequest {
    walletId: uint32
    validUntil: uint32
    seqno: uint32
    @abi.clientType(Cell<OutActionWithSendMessageOnly>?)
    outActions: OutActionsCell?
    hasExtraActions: bool
    extraActions: SnakedExtraActions
    // The signed request ends with a 512-bit Ed25519 signature.
}
