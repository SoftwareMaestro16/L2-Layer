// AUTO-GENERATED, do not edit
// It's a TypeScript wrapper for a AssetVault contract in Tolk.
/* eslint-disable */

import * as c from '@ton/core';
import { beginCell, ContractProvider, Sender, SendMode } from '@ton/core';

// ————————————————————————————————————————————
//   predefined types and functions
//

type StoreCallback<T> = (obj: T, b: c.Builder) => void
type LoadCallback<T> = (s: c.Slice) => T

export type CellRef<T> = {
    ref: T
}

function makeCellFrom<T>(self: T, storeFn_T: StoreCallback<T>): c.Cell {
    let b = beginCell();
    storeFn_T(self, b);
    return b.endCell();
}

function loadAndCheckPrefix32(s: c.Slice, expected: number, structName: string): void {
    let prefix = s.loadUint(32);
    if (prefix !== expected) {
        throw new Error(`Incorrect prefix for '${structName}': expected 0x${expected.toString(16).padStart(8, '0')}, got 0x${prefix.toString(16).padStart(8, '0')}`);
    }
}

function lookupPrefix(s: c.Slice, expected: number, prefixLen: number): boolean {
    return s.remainingBits >= prefixLen && s.preloadUint(prefixLen) === expected;
}

function throwNonePrefixMatch(fieldPath: string): never {
    throw new Error(`Incorrect prefix for '${fieldPath}': none of variants matched`);
}

function storeCellRef<T>(cell: CellRef<T>, b: c.Builder, storeFn_T: StoreCallback<T>): void {
    let b_ref = c.beginCell();
    storeFn_T(cell.ref, b_ref);
    b.storeRef(b_ref.endCell());
}

function loadCellRef<T>(s: c.Slice, loadFn_T: LoadCallback<T>): CellRef<T> {
    let s_ref = s.loadRef().beginParse();
    return { ref: loadFn_T(s_ref) };
}

function storeTolkNullable<T>(v: T | null, b: c.Builder, storeFn_T: StoreCallback<T>): void {
    if (v === null) {
        b.storeUint(0, 1);
    } else {
        b.storeUint(1, 1);
        storeFn_T(v, b);
    }
}

function createDictionaryValue<V>(loadFn_V: LoadCallback<V>, storeFn_V: StoreCallback<V>): c.DictionaryValue<V> {
    return {
        serialize(self: V, b: c.Builder) {
            storeFn_V(self, b);
        },
        parse(s: c.Slice): V {
            const value = loadFn_V(s);
            s.endParse();
            return value;
        }
    }
}

// ————————————————————————————————————————————
//   parse get methods result from a TVM stack
//

class StackReader {
    constructor(private tuple: c.TupleItem[]) {
    }

    static fromGetMethod(expectedN: number, getMethodResult: { stack: c.TupleReader }): StackReader {
        let tuple = [] as c.TupleItem[];
        while (getMethodResult.stack.remaining) {
            tuple.push(getMethodResult.stack.pop());
        }
        if (tuple.length !== expectedN) {
            throw new Error(`expected ${expectedN} stack width, got ${tuple.length}`);
        }
        return new StackReader(tuple);
    }

    private popExpecting<ItemT>(itemType: string): ItemT {
        const item = this.tuple.shift();
        if (item?.type === itemType) {
            return item as ItemT;
        }
        throw new Error(`not '${itemType}' on a stack`);
    }

    private popCellLike(): c.Cell {
        const item = this.tuple.shift();
        if (item && (item.type === 'cell' || item.type === 'slice' || item.type === 'builder')) {
            return item.cell;
        }
        throw new Error(`not cell/slice on a stack`);
    }

    readBigInt(): bigint {
        return this.popExpecting<c.TupleItemInt>('int').value;
    }

    readBoolean(): boolean {
        return this.popExpecting<c.TupleItemInt>('int').value !== 0n;
    }

    readCell(): c.Cell {
        return this.popCellLike();
    }

    readSlice(): c.Slice {
        return this.popCellLike().beginParse();
    }
}

// ————————————————————————————————————————————
//   auto-generated serializers to/from cells
//

type coins = bigint

type uint32 = bigint
type uint64 = bigint
type uint256 = bigint

/**
 > struct (0x4c324405) DepositTon {
 >     queryId: uint64
 >     amount: coins
 >     l2Recipient: uint256
 > }
 */
export interface DepositTon {
    readonly $: 'DepositTon'
    queryId: uint64
    amount: coins
    l2Recipient: uint256
}

export const DepositTon = {
    PREFIX: 0x4c324405,

    create(args: {
        queryId: uint64
        amount: coins
        l2Recipient: uint256
    }): DepositTon {
        return {
            $: 'DepositTon',
            ...args
        }
    },
    fromSlice(s: c.Slice): DepositTon {
        loadAndCheckPrefix32(s, 0x4c324405, 'DepositTon');
        return {
            $: 'DepositTon',
            queryId: s.loadUintBig(64),
            amount: s.loadCoins(),
            l2Recipient: s.loadUintBig(256),
        }
    },
    store(self: DepositTon, b: c.Builder): void {
        b.storeUint(0x4c324405, 32);
        b.storeUint(self.queryId, 64);
        b.storeCoins(self.amount);
        b.storeUint(self.l2Recipient, 256);
    },
    toCell(self: DepositTon): c.Cell {
        return makeCellFrom<DepositTon>(self, DepositTon.store);
    }
}

/**
 > struct (0x4c325206) ReleaseAuthorized {
 >     withdrawalId: uint256
 >     assetId: uint32
 >     recipient: address
 >     amount: coins
 > }
 */
export interface ReleaseAuthorized {
    readonly $: 'ReleaseAuthorized'
    withdrawalId: uint256
    assetId: uint32
    recipient: c.Address
    amount: coins
}

export const ReleaseAuthorized = {
    PREFIX: 0x4c325206,

    create(args: {
        withdrawalId: uint256
        assetId: uint32
        recipient: c.Address
        amount: coins
    }): ReleaseAuthorized {
        return {
            $: 'ReleaseAuthorized',
            ...args
        }
    },
    fromSlice(s: c.Slice): ReleaseAuthorized {
        loadAndCheckPrefix32(s, 0x4c325206, 'ReleaseAuthorized');
        return {
            $: 'ReleaseAuthorized',
            withdrawalId: s.loadUintBig(256),
            assetId: s.loadUintBig(32),
            recipient: s.loadAddress(),
            amount: s.loadCoins(),
        }
    },
    store(self: ReleaseAuthorized, b: c.Builder): void {
        b.storeUint(0x4c325206, 32);
        b.storeUint(self.withdrawalId, 256);
        b.storeUint(self.assetId, 32);
        b.storeAddress(self.recipient);
        b.storeCoins(self.amount);
    },
    toCell(self: ReleaseAuthorized): c.Cell {
        return makeCellFrom<ReleaseAuthorized>(self, ReleaseAuthorized.store);
    }
}

/**
 > struct DepositRecordedExtra {
 >     l1Sender: address
 > }
 */
export interface DepositRecordedExtra {
    readonly $: 'DepositRecordedExtra'
    l1Sender: c.Address
}

export const DepositRecordedExtra = {
    create(args: {
        l1Sender: c.Address
    }): DepositRecordedExtra {
        return {
            $: 'DepositRecordedExtra',
            ...args
        }
    },
    fromSlice(s: c.Slice): DepositRecordedExtra {
        return {
            $: 'DepositRecordedExtra',
            l1Sender: s.loadAddress(),
        }
    },
    store(self: DepositRecordedExtra, b: c.Builder): void {
        b.storeAddress(self.l1Sender);
    },
    toCell(self: DepositRecordedExtra): c.Cell {
        return makeCellFrom<DepositRecordedExtra>(self, DepositRecordedExtra.store);
    }
}

/**
 > struct (0x4c324407) DepositRecorded {
 >     queryId: uint64
 >     depositId: uint256
 >     assetId: uint32
 >     amount: coins
 >     l2Recipient: uint256
 >     extra: Cell<DepositRecordedExtra>
 > }
 */
export interface DepositRecorded {
    readonly $: 'DepositRecorded'
    queryId: uint64
    depositId: uint256
    assetId: uint32
    amount: coins
    l2Recipient: uint256
    extra: CellRef<DepositRecordedExtra>
}

export const DepositRecorded = {
    PREFIX: 0x4c324407,

    create(args: {
        queryId: uint64
        depositId: uint256
        assetId: uint32
        amount: coins
        l2Recipient: uint256
        extra: CellRef<DepositRecordedExtra>
    }): DepositRecorded {
        return {
            $: 'DepositRecorded',
            ...args
        }
    },
    fromSlice(s: c.Slice): DepositRecorded {
        loadAndCheckPrefix32(s, 0x4c324407, 'DepositRecorded');
        return {
            $: 'DepositRecorded',
            queryId: s.loadUintBig(64),
            depositId: s.loadUintBig(256),
            assetId: s.loadUintBig(32),
            amount: s.loadCoins(),
            l2Recipient: s.loadUintBig(256),
            extra: loadCellRef<DepositRecordedExtra>(s, DepositRecordedExtra.fromSlice),
        }
    },
    store(self: DepositRecorded, b: c.Builder): void {
        b.storeUint(0x4c324407, 32);
        b.storeUint(self.queryId, 64);
        b.storeUint(self.depositId, 256);
        b.storeUint(self.assetId, 32);
        b.storeCoins(self.amount);
        b.storeUint(self.l2Recipient, 256);
        storeCellRef<DepositRecordedExtra>(self.extra, b, DepositRecordedExtra.store);
    },
    toCell(self: DepositRecorded): c.Cell {
        return makeCellFrom<DepositRecorded>(self, DepositRecorded.store);
    }
}

/**
 > struct (0x4c325208) ReleaseFailed {
 >     withdrawalId: uint256
 >     assetId: uint32
 >     recipient: address
 >     amount: coins
 > }
 */
export interface ReleaseFailed {
    readonly $: 'ReleaseFailed'
    withdrawalId: uint256
    assetId: uint32
    recipient: c.Address
    amount: coins
}

export const ReleaseFailed = {
    PREFIX: 0x4c325208,

    create(args: {
        withdrawalId: uint256
        assetId: uint32
        recipient: c.Address
        amount: coins
    }): ReleaseFailed {
        return {
            $: 'ReleaseFailed',
            ...args
        }
    },
    fromSlice(s: c.Slice): ReleaseFailed {
        loadAndCheckPrefix32(s, 0x4c325208, 'ReleaseFailed');
        return {
            $: 'ReleaseFailed',
            withdrawalId: s.loadUintBig(256),
            assetId: s.loadUintBig(32),
            recipient: s.loadAddress(),
            amount: s.loadCoins(),
        }
    },
    store(self: ReleaseFailed, b: c.Builder): void {
        b.storeUint(0x4c325208, 32);
        b.storeUint(self.withdrawalId, 256);
        b.storeUint(self.assetId, 32);
        b.storeAddress(self.recipient);
        b.storeCoins(self.amount);
    },
    toCell(self: ReleaseFailed): c.Cell {
        return makeCellFrom<ReleaseFailed>(self, ReleaseFailed.store);
    }
}

/**
 > struct (0x7362d09c) JettonTransferNotification {
 >     queryId: uint64
 >     amount: coins
 >     sender: address
 >     forwardPayload: cell
 > }
 */
export interface JettonTransferNotification {
    readonly $: 'JettonTransferNotification'
    queryId: uint64
    amount: coins
    sender: c.Address
    forwardPayload: c.Cell
}

export const JettonTransferNotification = {
    PREFIX: 0x7362d09c,

    create(args: {
        queryId: uint64
        amount: coins
        sender: c.Address
        forwardPayload: c.Cell
    }): JettonTransferNotification {
        return {
            $: 'JettonTransferNotification',
            ...args
        }
    },
    fromSlice(s: c.Slice): JettonTransferNotification {
        loadAndCheckPrefix32(s, 0x7362d09c, 'JettonTransferNotification');
        return {
            $: 'JettonTransferNotification',
            queryId: s.loadUintBig(64),
            amount: s.loadCoins(),
            sender: s.loadAddress(),
            forwardPayload: s.loadRef(),
        }
    },
    store(self: JettonTransferNotification, b: c.Builder): void {
        b.storeUint(0x7362d09c, 32);
        b.storeUint(self.queryId, 64);
        b.storeCoins(self.amount);
        b.storeAddress(self.sender);
        b.storeRef(self.forwardPayload);
    },
    toCell(self: JettonTransferNotification): c.Cell {
        return makeCellFrom<JettonTransferNotification>(self, JettonTransferNotification.store);
    }
}

/**
 > struct AssetVaultStorage {
 >     admin: address
 >     rollupRoot: address
 >     wrappedGasMinter: address
 >     paused: bool
 >     lockedTon: coins
 >     releaseFailures: map<uint256, Cell<ReleaseFailed>>
 > }
 */
export interface AssetVaultStorage {
    readonly $: 'AssetVaultStorage'
    admin: c.Address
    rollupRoot: c.Address
    wrappedGasMinter: c.Address
    paused: boolean
    lockedTon: coins
    releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailed>>
}

export const AssetVaultStorage = {
    create(args: {
        admin: c.Address
        rollupRoot: c.Address
        wrappedGasMinter: c.Address
        paused: boolean
        lockedTon: coins
        releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailed>>
    }): AssetVaultStorage {
        return {
            $: 'AssetVaultStorage',
            ...args
        }
    },
    fromSlice(s: c.Slice): AssetVaultStorage {
        return {
            $: 'AssetVaultStorage',
            admin: s.loadAddress(),
            rollupRoot: s.loadAddress(),
            wrappedGasMinter: s.loadAddress(),
            paused: s.loadBoolean(),
            lockedTon: s.loadCoins(),
            releaseFailures: c.Dictionary.load<uint256, CellRef<ReleaseFailed>>(c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailed>>(
                (s) => loadCellRef<ReleaseFailed>(s, ReleaseFailed.fromSlice),
                (v,b) => storeCellRef<ReleaseFailed>(v, b, ReleaseFailed.store)
            ), s),
        }
    },
    store(self: AssetVaultStorage, b: c.Builder): void {
        b.storeAddress(self.admin);
        b.storeAddress(self.rollupRoot);
        b.storeAddress(self.wrappedGasMinter);
        b.storeBit(self.paused);
        b.storeCoins(self.lockedTon);
        b.storeDict<uint256, CellRef<ReleaseFailed>>(self.releaseFailures, c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailed>>(
            (s) => loadCellRef<ReleaseFailed>(s, ReleaseFailed.fromSlice),
            (v,b) => storeCellRef<ReleaseFailed>(v, b, ReleaseFailed.store)
        ));
    },
    toCell(self: AssetVaultStorage): c.Cell {
        return makeCellFrom<AssetVaultStorage>(self, AssetVaultStorage.store);
    }
}

/**
 > struct VaultStatusReply {
 >     rollupRoot: address
 >     wrappedGasMinter: address
 >     lockedTon: coins
 >     paused: bool
 > }
 */
export interface VaultStatusReply {
    readonly $: 'VaultStatusReply'
    rollupRoot: c.Address
    wrappedGasMinter: c.Address
    lockedTon: coins
    paused: boolean
}

export const VaultStatusReply = {
    create(args: {
        rollupRoot: c.Address
        wrappedGasMinter: c.Address
        lockedTon: coins
        paused: boolean
    }): VaultStatusReply {
        return {
            $: 'VaultStatusReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): VaultStatusReply {
        return {
            $: 'VaultStatusReply',
            rollupRoot: s.loadAddress(),
            wrappedGasMinter: s.loadAddress(),
            lockedTon: s.loadCoins(),
            paused: s.loadBoolean(),
        }
    },
    store(self: VaultStatusReply, b: c.Builder): void {
        b.storeAddress(self.rollupRoot);
        b.storeAddress(self.wrappedGasMinter);
        b.storeCoins(self.lockedTon);
        b.storeBit(self.paused);
    },
    toCell(self: VaultStatusReply): c.Cell {
        return makeCellFrom<VaultStatusReply>(self, VaultStatusReply.store);
    }
}

// ————————————————————————————————————————————
//    class AssetVault
//

interface ExtraSendOptions {
    bounce?: boolean                    // default: false
    sendMode?: SendMode                 // default: SendMode.PAY_GAS_SEPARATELY
    extraCurrencies?: c.ExtraCurrency   // default: empty dict
}

interface DeployedAddrOptions {
    workchain?: number                  // default: 0 (basechain)
    toShard?: { fixedPrefixLength: number; closeTo: c.Address }
    overrideContractCode?: c.Cell
}

function calculateDeployedAddress(code: c.Cell, data: c.Cell, options: DeployedAddrOptions): c.Address {
    const stateInitCell = beginCell().store(c.storeStateInit({
        code,
        data,
        splitDepth: options.toShard?.fixedPrefixLength,
        special: null,
        libraries: null,
    })).endCell();

    let addrHash = stateInitCell.hash();
    if (options.toShard) {
        const shardDepth = options.toShard.fixedPrefixLength;
        addrHash = beginCell()
            .storeBits(new c.BitString(options.toShard.closeTo.hash, 0, shardDepth))
            .storeBits(new c.BitString(stateInitCell.hash(), shardDepth, 256 - shardDepth))
            .endCell()
            .beginParse().loadBuffer(32);
    }

    return new c.Address(options.workchain ?? 0, addrHash);
}

export class AssetVault implements c.Contract {
    static CodeCell = c.Cell.fromBase64('te6ccgECCwEAAkoAART/APSkE/S88sgLAQIBYgIDAgLPBAUAJaGnE9qJofSQY/SR9JGkAfQAYAME9T4kY5Y0x8x1ywiYZKQNI5K7UTQAdP/0x/6SPoAMAT6SPpI+kjWAPoA9AXIz5EwyUgiKc8L/xjLHxb6UlAI+gLJQGWDB/QXAcj6UhP6UhP6UhPOAfoC9ADJ7VTgMOAg1ywiYZIgLOMC1ywiYZKQNOMC1ywjmxaE5OMCMIAYHCAkB9ztRND6SPpI+kjSAPoA9AWBEAIjs/L0gRABUbXHBRvy9CfAAY45NzeBEAtTZL7y9FFToQLI+lL6UhT6UhTKAFj6AhP0AMntVMjPhYgS+lIB+gLPgXP6AnDPC2XJcPsA4MjPkTDJSCIpzwv/GMsfFvpSUAT6AslAZ4MH9BeAKAP4x0z/6ANcL//iS+JftRND6SPpI+kjSAPoAgRACI7Py9IEQC1F6vhfy9CigBMj6UhP6UvpSygAB+gLOye1UIMj6UiTPCz8j+gIizwv/+RaLAgLI+lLJyM+RMMkQHhbLP8v/z5AAAAAGUAP6Asv/EszJyM+HIBLOcc8LYczJcPsAACAx0//TH/pI+gAw+JJVMPABANIx0z/6APpI10z4ku1E0PpIMfpIMfpIMdcKAIEQAgGz8vQB0NcL/wHI+lIkzws/I/oCIc8L//kWiwIDyPpSycjPkTDJEB4Wyz/L/8+QAAAAClAD+gISy/8SzMnIz4cgEs5xzwthzMlw+wAADoQPAccA8vQAKgXI+lIU+lIT+lISygAB+gL0AMntVA==');

    static Errors = {
        'Errors.Unauthorized': 4097,
        'Errors.Paused': 4098,
        'Errors.BadDepositValue': 4107,
        'Errors.UnknownOpcode': 65535,
    }

    readonly address: c.Address
    readonly init: { code: c.Cell, data: c.Cell } | undefined

    protected constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new AssetVault(address);
    }

    static fromStorage(emptyStorage: {
        admin: c.Address
        rollupRoot: c.Address
        wrappedGasMinter: c.Address
        paused: boolean
        lockedTon: coins
        releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailed>>
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? AssetVault.CodeCell,
            data: AssetVaultStorage.toCell(AssetVaultStorage.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new AssetVault(address, initialState);
    }

    static createCellOfDepositTon(body: {
        queryId: uint64
        amount: coins
        l2Recipient: uint256
    }) {
        return DepositTon.toCell(DepositTon.create(body));
    }

    static createCellOfReleaseAuthorized(body: {
        withdrawalId: uint256
        assetId: uint32
        recipient: c.Address
        amount: coins
    }) {
        return ReleaseAuthorized.toCell(ReleaseAuthorized.create(body));
    }

    static createCellOfJettonTransferNotification(body: {
        queryId: uint64
        amount: coins
        sender: c.Address
        forwardPayload: c.Cell
    }) {
        return JettonTransferNotification.toCell(JettonTransferNotification.create(body));
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendDepositTon(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
        amount: coins
        l2Recipient: uint256
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: DepositTon.toCell(DepositTon.create(body)),
            ...extraOptions
        });
    }

    async sendReleaseAuthorized(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        withdrawalId: uint256
        assetId: uint32
        recipient: c.Address
        amount: coins
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ReleaseAuthorized.toCell(ReleaseAuthorized.create(body)),
            ...extraOptions
        });
    }

    async sendJettonTransferNotification(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
        amount: coins
        sender: c.Address
        forwardPayload: c.Cell
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: JettonTransferNotification.toCell(JettonTransferNotification.create(body)),
            ...extraOptions
        });
    }

    async getVaultStatus(provider: ContractProvider): Promise<VaultStatusReply> {
        const r = StackReader.fromGetMethod(4, await provider.get('vaultStatus', []));
        return ({
            $: 'VaultStatusReply',
            rollupRoot: r.readSlice().loadAddress(),
            wrappedGasMinter: r.readSlice().loadAddress(),
            lockedTon: r.readBigInt(),
            paused: r.readBoolean(),
        });
    }
}
