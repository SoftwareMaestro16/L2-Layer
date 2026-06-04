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

    readCellRef<T>(loadFn_T: LoadCallback<T>): CellRef<T> {
        return { ref: loadFn_T(this.readCell().beginParse()) };
    }
}

// ————————————————————————————————————————————
//   auto-generated serializers to/from cells
//

type coins = bigint

type uint8 = bigint
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
 > struct (0x4c325207) RetryRelease {
 >     withdrawalId: uint256
 > }
 */
export interface RetryRelease {
    readonly $: 'RetryRelease'
    withdrawalId: uint256
}

export const RetryRelease = {
    PREFIX: 0x4c325207,

    create(args: {
        withdrawalId: uint256
    }): RetryRelease {
        return {
            $: 'RetryRelease',
            ...args
        }
    },
    fromSlice(s: c.Slice): RetryRelease {
        loadAndCheckPrefix32(s, 0x4c325207, 'RetryRelease');
        return {
            $: 'RetryRelease',
            withdrawalId: s.loadUintBig(256),
        }
    },
    store(self: RetryRelease, b: c.Builder): void {
        b.storeUint(0x4c325207, 32);
        b.storeUint(self.withdrawalId, 256);
    },
    toCell(self: RetryRelease): c.Cell {
        return makeCellFrom<RetryRelease>(self, RetryRelease.store);
    }
}

/**
 > struct (0x4c32450b) RegisterJettonAsset {
 >     assetId: uint32
 >     master: address
 >     wallet: address
 >     decimals: uint8
 > }
 */
export interface RegisterJettonAsset {
    readonly $: 'RegisterJettonAsset'
    assetId: uint32
    master: c.Address
    wallet: c.Address
    decimals: uint8
}

export const RegisterJettonAsset = {
    PREFIX: 0x4c32450b,

    create(args: {
        assetId: uint32
        master: c.Address
        wallet: c.Address
        decimals: uint8
    }): RegisterJettonAsset {
        return {
            $: 'RegisterJettonAsset',
            ...args
        }
    },
    fromSlice(s: c.Slice): RegisterJettonAsset {
        loadAndCheckPrefix32(s, 0x4c32450b, 'RegisterJettonAsset');
        return {
            $: 'RegisterJettonAsset',
            assetId: s.loadUintBig(32),
            master: s.loadAddress(),
            wallet: s.loadAddress(),
            decimals: s.loadUintBig(8),
        }
    },
    store(self: RegisterJettonAsset, b: c.Builder): void {
        b.storeUint(0x4c32450b, 32);
        b.storeUint(self.assetId, 32);
        b.storeAddress(self.master);
        b.storeAddress(self.wallet);
        b.storeUint(self.decimals, 8);
    },
    toCell(self: RegisterJettonAsset): c.Cell {
        return makeCellFrom<RegisterJettonAsset>(self, RegisterJettonAsset.store);
    }
}

/**
 > struct ReleaseFailure {
 >     withdrawalId: uint256
 >     assetId: uint32
 >     recipient: address
 >     amount: coins
 >     reason: uint8
 >     failedAt: uint32
 >     retryCount: uint32
 > }
 */
export interface ReleaseFailure {
    readonly $: 'ReleaseFailure'
    withdrawalId: uint256
    assetId: uint32
    recipient: c.Address
    amount: coins
    reason: uint8
    failedAt: uint32
    retryCount: uint32
}

export const ReleaseFailure = {
    create(args: {
        withdrawalId: uint256
        assetId: uint32
        recipient: c.Address
        amount: coins
        reason: uint8
        failedAt: uint32
        retryCount: uint32
    }): ReleaseFailure {
        return {
            $: 'ReleaseFailure',
            ...args
        }
    },
    fromSlice(s: c.Slice): ReleaseFailure {
        return {
            $: 'ReleaseFailure',
            withdrawalId: s.loadUintBig(256),
            assetId: s.loadUintBig(32),
            recipient: s.loadAddress(),
            amount: s.loadCoins(),
            reason: s.loadUintBig(8),
            failedAt: s.loadUintBig(32),
            retryCount: s.loadUintBig(32),
        }
    },
    store(self: ReleaseFailure, b: c.Builder): void {
        b.storeUint(self.withdrawalId, 256);
        b.storeUint(self.assetId, 32);
        b.storeAddress(self.recipient);
        b.storeCoins(self.amount);
        b.storeUint(self.reason, 8);
        b.storeUint(self.failedAt, 32);
        b.storeUint(self.retryCount, 32);
    },
    toCell(self: ReleaseFailure): c.Cell {
        return makeCellFrom<ReleaseFailure>(self, ReleaseFailure.store);
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
 > struct (0xd53276db) JettonExcesses {
 >     queryId: uint64
 > }
 */
export interface JettonExcesses {
    readonly $: 'JettonExcesses'
    queryId: uint64
}

export const JettonExcesses = {
    PREFIX: 0xd53276db,

    create(args: {
        queryId: uint64
    }): JettonExcesses {
        return {
            $: 'JettonExcesses',
            ...args
        }
    },
    fromSlice(s: c.Slice): JettonExcesses {
        loadAndCheckPrefix32(s, 0xd53276db, 'JettonExcesses');
        return {
            $: 'JettonExcesses',
            queryId: s.loadUintBig(64),
        }
    },
    store(self: JettonExcesses, b: c.Builder): void {
        b.storeUint(0xd53276db, 32);
        b.storeUint(self.queryId, 64);
    },
    toCell(self: JettonExcesses): c.Cell {
        return makeCellFrom<JettonExcesses>(self, JettonExcesses.store);
    }
}

/**
 > struct JettonAssetConfig {
 >     assetId: uint32
 >     master: address
 >     wallet: address
 >     decimals: uint8
 > }
 */
export interface JettonAssetConfig {
    readonly $: 'JettonAssetConfig'
    assetId: uint32
    master: c.Address
    wallet: c.Address
    decimals: uint8
}

export const JettonAssetConfig = {
    create(args: {
        assetId: uint32
        master: c.Address
        wallet: c.Address
        decimals: uint8
    }): JettonAssetConfig {
        return {
            $: 'JettonAssetConfig',
            ...args
        }
    },
    fromSlice(s: c.Slice): JettonAssetConfig {
        return {
            $: 'JettonAssetConfig',
            assetId: s.loadUintBig(32),
            master: s.loadAddress(),
            wallet: s.loadAddress(),
            decimals: s.loadUintBig(8),
        }
    },
    store(self: JettonAssetConfig, b: c.Builder): void {
        b.storeUint(self.assetId, 32);
        b.storeAddress(self.master);
        b.storeAddress(self.wallet);
        b.storeUint(self.decimals, 8);
    },
    toCell(self: JettonAssetConfig): c.Cell {
        return makeCellFrom<JettonAssetConfig>(self, JettonAssetConfig.store);
    }
}

/**
 > struct AssetVaultStorage {
 >     admin: address
 >     rollupRoot: address
 >     wrappedGasMinter: address
 >     paused: bool
 >     lockedTon: coins
 >     tonAssetId: uint32
 >     tonDecimals: uint8
 >     jettonAssets: map<uint32, Cell<JettonAssetConfig>>
 >     jettonWalletToAsset: map<uint256, uint32>
 >     pendingJettonReleases: map<uint64, Cell<ReleaseAuthorized>>
 >     releaseFailures: map<uint256, Cell<ReleaseFailure>>
 > }
 */
export interface AssetVaultStorage {
    readonly $: 'AssetVaultStorage'
    admin: c.Address
    rollupRoot: c.Address
    wrappedGasMinter: c.Address
    paused: boolean
    lockedTon: coins
    tonAssetId: uint32
    tonDecimals: uint8
    jettonAssets: c.Dictionary<uint32, CellRef<JettonAssetConfig>>
    jettonWalletToAsset: c.Dictionary<uint256, uint32>
    pendingJettonReleases: c.Dictionary<uint64, CellRef<ReleaseAuthorized>>
    releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailure>>
}

export const AssetVaultStorage = {
    create(args: {
        admin: c.Address
        rollupRoot: c.Address
        wrappedGasMinter: c.Address
        paused: boolean
        lockedTon: coins
        tonAssetId: uint32
        tonDecimals: uint8
        jettonAssets: c.Dictionary<uint32, CellRef<JettonAssetConfig>>
        jettonWalletToAsset: c.Dictionary<uint256, uint32>
        pendingJettonReleases: c.Dictionary<uint64, CellRef<ReleaseAuthorized>>
        releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailure>>
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
            tonAssetId: s.loadUintBig(32),
            tonDecimals: s.loadUintBig(8),
            jettonAssets: c.Dictionary.load<uint32, CellRef<JettonAssetConfig>>(c.Dictionary.Keys.BigUint(32), createDictionaryValue<CellRef<JettonAssetConfig>>(
                (s) => loadCellRef<JettonAssetConfig>(s, JettonAssetConfig.fromSlice),
                (v,b) => storeCellRef<JettonAssetConfig>(v, b, JettonAssetConfig.store)
            ), s),
            jettonWalletToAsset: c.Dictionary.load<uint256, uint32>(c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.BigUint(32), s),
            pendingJettonReleases: c.Dictionary.load<uint64, CellRef<ReleaseAuthorized>>(c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<ReleaseAuthorized>>(
                (s) => loadCellRef<ReleaseAuthorized>(s, ReleaseAuthorized.fromSlice),
                (v,b) => storeCellRef<ReleaseAuthorized>(v, b, ReleaseAuthorized.store)
            ), s),
            releaseFailures: c.Dictionary.load<uint256, CellRef<ReleaseFailure>>(c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailure>>(
                (s) => loadCellRef<ReleaseFailure>(s, ReleaseFailure.fromSlice),
                (v,b) => storeCellRef<ReleaseFailure>(v, b, ReleaseFailure.store)
            ), s),
        }
    },
    store(self: AssetVaultStorage, b: c.Builder): void {
        b.storeAddress(self.admin);
        b.storeAddress(self.rollupRoot);
        b.storeAddress(self.wrappedGasMinter);
        b.storeBit(self.paused);
        b.storeCoins(self.lockedTon);
        b.storeUint(self.tonAssetId, 32);
        b.storeUint(self.tonDecimals, 8);
        b.storeDict<uint32, CellRef<JettonAssetConfig>>(self.jettonAssets, c.Dictionary.Keys.BigUint(32), createDictionaryValue<CellRef<JettonAssetConfig>>(
            (s) => loadCellRef<JettonAssetConfig>(s, JettonAssetConfig.fromSlice),
            (v,b) => storeCellRef<JettonAssetConfig>(v, b, JettonAssetConfig.store)
        ));
        b.storeDict<uint256, uint32>(self.jettonWalletToAsset, c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.BigUint(32));
        b.storeDict<uint64, CellRef<ReleaseAuthorized>>(self.pendingJettonReleases, c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<ReleaseAuthorized>>(
            (s) => loadCellRef<ReleaseAuthorized>(s, ReleaseAuthorized.fromSlice),
            (v,b) => storeCellRef<ReleaseAuthorized>(v, b, ReleaseAuthorized.store)
        ));
        b.storeDict<uint256, CellRef<ReleaseFailure>>(self.releaseFailures, c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailure>>(
            (s) => loadCellRef<ReleaseFailure>(s, ReleaseFailure.fromSlice),
            (v,b) => storeCellRef<ReleaseFailure>(v, b, ReleaseFailure.store)
        ));
    },
    toCell(self: AssetVaultStorage): c.Cell {
        return makeCellFrom<AssetVaultStorage>(self, AssetVaultStorage.store);
    }
}

/**
 > struct ReleaseFailureReply {
 >     exists: bool
 >     failure: Cell<ReleaseFailure>
 > }
 */
export interface ReleaseFailureReply {
    readonly $: 'ReleaseFailureReply'
    exists: boolean
    failure: CellRef<ReleaseFailure>
}

export const ReleaseFailureReply = {
    create(args: {
        exists: boolean
        failure: CellRef<ReleaseFailure>
    }): ReleaseFailureReply {
        return {
            $: 'ReleaseFailureReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): ReleaseFailureReply {
        return {
            $: 'ReleaseFailureReply',
            exists: s.loadBoolean(),
            failure: loadCellRef<ReleaseFailure>(s, ReleaseFailure.fromSlice),
        }
    },
    store(self: ReleaseFailureReply, b: c.Builder): void {
        b.storeBit(self.exists);
        storeCellRef<ReleaseFailure>(self.failure, b, ReleaseFailure.store);
    },
    toCell(self: ReleaseFailureReply): c.Cell {
        return makeCellFrom<ReleaseFailureReply>(self, ReleaseFailureReply.store);
    }
}

/**
 > struct VaultStatusReply {
 >     rollupRoot: address
 >     wrappedGasMinter: address
 >     lockedTon: coins
 >     tonAssetId: uint32
 >     tonDecimals: uint8
 >     paused: bool
 > }
 */
export interface VaultStatusReply {
    readonly $: 'VaultStatusReply'
    rollupRoot: c.Address
    wrappedGasMinter: c.Address
    lockedTon: coins
    tonAssetId: uint32
    tonDecimals: uint8
    paused: boolean
}

export const VaultStatusReply = {
    create(args: {
        rollupRoot: c.Address
        wrappedGasMinter: c.Address
        lockedTon: coins
        tonAssetId: uint32
        tonDecimals: uint8
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
            tonAssetId: s.loadUintBig(32),
            tonDecimals: s.loadUintBig(8),
            paused: s.loadBoolean(),
        }
    },
    store(self: VaultStatusReply, b: c.Builder): void {
        b.storeAddress(self.rollupRoot);
        b.storeAddress(self.wrappedGasMinter);
        b.storeCoins(self.lockedTon);
        b.storeUint(self.tonAssetId, 32);
        b.storeUint(self.tonDecimals, 8);
        b.storeBit(self.paused);
    },
    toCell(self: VaultStatusReply): c.Cell {
        return makeCellFrom<VaultStatusReply>(self, VaultStatusReply.store);
    }
}

/**
 > struct JettonAssetReply {
 >     exists: bool
 >     asset: Cell<JettonAssetConfig>
 > }
 */
export interface JettonAssetReply {
    readonly $: 'JettonAssetReply'
    exists: boolean
    asset: CellRef<JettonAssetConfig>
}

export const JettonAssetReply = {
    create(args: {
        exists: boolean
        asset: CellRef<JettonAssetConfig>
    }): JettonAssetReply {
        return {
            $: 'JettonAssetReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): JettonAssetReply {
        return {
            $: 'JettonAssetReply',
            exists: s.loadBoolean(),
            asset: loadCellRef<JettonAssetConfig>(s, JettonAssetConfig.fromSlice),
        }
    },
    store(self: JettonAssetReply, b: c.Builder): void {
        b.storeBit(self.exists);
        storeCellRef<JettonAssetConfig>(self.asset, b, JettonAssetConfig.store);
    },
    toCell(self: JettonAssetReply): c.Cell {
        return makeCellFrom<JettonAssetReply>(self, JettonAssetReply.store);
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
    static CodeCell = c.Cell.fromBase64('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');

    static Errors = {
        'Errors.Unauthorized': 4097,
        'Errors.Paused': 4098,
        'Errors.BadDepositValue': 4107,
        'Errors.NoFailedWithdrawal': 4109,
        'Errors.UnsupportedReleaseAsset': 4110,
        'Errors.BadJettonWallet': 4111,
        'Errors.BadForwardPayload': 4112,
        'Errors.BadJettonAmount': 4113,
        'Errors.BadAssetDecimals': 4114,
        'Errors.BadAssetConfig': 4115,
        'Errors.BadJettonQueryId': 4117,
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
        tonAssetId: uint32
        tonDecimals: uint8
        jettonAssets: c.Dictionary<uint32, CellRef<JettonAssetConfig>>
        jettonWalletToAsset: c.Dictionary<uint256, uint32>
        pendingJettonReleases: c.Dictionary<uint64, CellRef<ReleaseAuthorized>>
        releaseFailures: c.Dictionary<uint256, CellRef<ReleaseFailure>>
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

    static createCellOfRetryRelease(body: {
        withdrawalId: uint256
    }) {
        return RetryRelease.toCell(RetryRelease.create(body));
    }

    static createCellOfRegisterJettonAsset(body: {
        assetId: uint32
        master: c.Address
        wallet: c.Address
        decimals: uint8
    }) {
        return RegisterJettonAsset.toCell(RegisterJettonAsset.create(body));
    }

    static createCellOfJettonTransferNotification(body: {
        queryId: uint64
        amount: coins
        sender: c.Address
        forwardPayload: c.Cell
    }) {
        return JettonTransferNotification.toCell(JettonTransferNotification.create(body));
    }

    static createCellOfJettonExcesses(body: {
        queryId: uint64
    }) {
        return JettonExcesses.toCell(JettonExcesses.create(body));
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

    async sendRetryRelease(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        withdrawalId: uint256
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: RetryRelease.toCell(RetryRelease.create(body)),
            ...extraOptions
        });
    }

    async sendRegisterJettonAsset(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        assetId: uint32
        master: c.Address
        wallet: c.Address
        decimals: uint8
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: RegisterJettonAsset.toCell(RegisterJettonAsset.create(body)),
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

    async sendJettonExcesses(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        queryId: uint64
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: JettonExcesses.toCell(JettonExcesses.create(body)),
            ...extraOptions
        });
    }

    async getVaultStatus(provider: ContractProvider): Promise<VaultStatusReply> {
        const r = StackReader.fromGetMethod(6, await provider.get('vaultStatus', []));
        return ({
            $: 'VaultStatusReply',
            rollupRoot: r.readSlice().loadAddress(),
            wrappedGasMinter: r.readSlice().loadAddress(),
            lockedTon: r.readBigInt(),
            tonAssetId: r.readBigInt(),
            tonDecimals: r.readBigInt(),
            paused: r.readBoolean(),
        });
    }

    async getFailedRelease(provider: ContractProvider, withdrawalId: uint256): Promise<ReleaseFailureReply> {
        const r = StackReader.fromGetMethod(2, await provider.get('failedRelease', [
            { type: 'int', value: withdrawalId },
        ]));
        return ({
            $: 'ReleaseFailureReply',
            exists: r.readBoolean(),
            failure: r.readCellRef<ReleaseFailure>(ReleaseFailure.fromSlice),
        });
    }

    async getJettonAsset(provider: ContractProvider, assetId: uint32): Promise<JettonAssetReply> {
        const r = StackReader.fromGetMethod(2, await provider.get('jettonAsset', [
            { type: 'int', value: assetId },
        ]));
        return ({
            $: 'JettonAssetReply',
            exists: r.readBoolean(),
            asset: r.readCellRef<JettonAssetConfig>(JettonAssetConfig.fromSlice),
        });
    }
}
