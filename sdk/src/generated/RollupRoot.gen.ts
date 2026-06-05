// AUTO-GENERATED, do not edit
// It's a TypeScript wrapper for a RollupRoot contract in Tolk.
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
 > struct BatchRootsA {
 >     prevStateRoot: uint256
 >     stateRoot: uint256
 >     txRoot: uint256
 > }
 */
export interface BatchRootsA {
    readonly $: 'BatchRootsA'
    prevStateRoot: uint256
    stateRoot: uint256
    txRoot: uint256
}

export const BatchRootsA = {
    create(args: {
        prevStateRoot: uint256
        stateRoot: uint256
        txRoot: uint256
    }): BatchRootsA {
        return {
            $: 'BatchRootsA',
            ...args
        }
    },
    fromSlice(s: c.Slice): BatchRootsA {
        return {
            $: 'BatchRootsA',
            prevStateRoot: s.loadUintBig(256),
            stateRoot: s.loadUintBig(256),
            txRoot: s.loadUintBig(256),
        }
    },
    store(self: BatchRootsA, b: c.Builder): void {
        b.storeUint(self.prevStateRoot, 256);
        b.storeUint(self.stateRoot, 256);
        b.storeUint(self.txRoot, 256);
    },
    toCell(self: BatchRootsA): c.Cell {
        return makeCellFrom<BatchRootsA>(self, BatchRootsA.store);
    }
}

/**
 > struct BatchRootsB {
 >     receiptRoot: uint256
 >     withdrawalRoot: uint256
 >     dataHash: uint256
 > }
 */
export interface BatchRootsB {
    readonly $: 'BatchRootsB'
    receiptRoot: uint256
    withdrawalRoot: uint256
    dataHash: uint256
}

export const BatchRootsB = {
    create(args: {
        receiptRoot: uint256
        withdrawalRoot: uint256
        dataHash: uint256
    }): BatchRootsB {
        return {
            $: 'BatchRootsB',
            ...args
        }
    },
    fromSlice(s: c.Slice): BatchRootsB {
        return {
            $: 'BatchRootsB',
            receiptRoot: s.loadUintBig(256),
            withdrawalRoot: s.loadUintBig(256),
            dataHash: s.loadUintBig(256),
        }
    },
    store(self: BatchRootsB, b: c.Builder): void {
        b.storeUint(self.receiptRoot, 256);
        b.storeUint(self.withdrawalRoot, 256);
        b.storeUint(self.dataHash, 256);
    },
    toCell(self: BatchRootsB): c.Cell {
        return makeCellFrom<BatchRootsB>(self, BatchRootsB.store);
    }
}

/**
 > struct BatchCommitment {
 >     rootsA: Cell<BatchRootsA>
 >     rootsB: Cell<BatchRootsB>
 >     committedAt: uint32
 >     finalized: bool
 > }
 */
export interface BatchCommitment {
    readonly $: 'BatchCommitment'
    rootsA: CellRef<BatchRootsA>
    rootsB: CellRef<BatchRootsB>
    committedAt: uint32
    finalized: boolean
}

export const BatchCommitment = {
    create(args: {
        rootsA: CellRef<BatchRootsA>
        rootsB: CellRef<BatchRootsB>
        committedAt: uint32
        finalized: boolean
    }): BatchCommitment {
        return {
            $: 'BatchCommitment',
            ...args
        }
    },
    fromSlice(s: c.Slice): BatchCommitment {
        return {
            $: 'BatchCommitment',
            rootsA: loadCellRef<BatchRootsA>(s, BatchRootsA.fromSlice),
            rootsB: loadCellRef<BatchRootsB>(s, BatchRootsB.fromSlice),
            committedAt: s.loadUintBig(32),
            finalized: s.loadBoolean(),
        }
    },
    store(self: BatchCommitment, b: c.Builder): void {
        storeCellRef<BatchRootsA>(self.rootsA, b, BatchRootsA.store);
        storeCellRef<BatchRootsB>(self.rootsB, b, BatchRootsB.store);
        b.storeUint(self.committedAt, 32);
        b.storeBit(self.finalized);
    },
    toCell(self: BatchCommitment): c.Cell {
        return makeCellFrom<BatchCommitment>(self, BatchCommitment.store);
    }
}

/**
 > struct BatchChallenge {
 >     batchNo: uint64
 >     challenger: address
 >     reason: uint8
 >     evidenceHash: uint256
 >     openedAt: uint32
 >     status: uint8
 >     challengerBond: coins
 >     slashedAmount: coins
 > }
 */
export interface BatchChallenge {
    readonly $: 'BatchChallenge'
    batchNo: uint64
    challenger: c.Address
    reason: uint8
    evidenceHash: uint256
    openedAt: uint32
    status: uint8
    challengerBond: coins
    slashedAmount: coins
}

export const BatchChallenge = {
    create(args: {
        batchNo: uint64
        challenger: c.Address
        reason: uint8
        evidenceHash: uint256
        openedAt: uint32
        status: uint8
        challengerBond: coins
        slashedAmount: coins
    }): BatchChallenge {
        return {
            $: 'BatchChallenge',
            ...args
        }
    },
    fromSlice(s: c.Slice): BatchChallenge {
        return {
            $: 'BatchChallenge',
            batchNo: s.loadUintBig(64),
            challenger: s.loadAddress(),
            reason: s.loadUintBig(8),
            evidenceHash: s.loadUintBig(256),
            openedAt: s.loadUintBig(32),
            status: s.loadUintBig(8),
            challengerBond: s.loadCoins(),
            slashedAmount: s.loadCoins(),
        }
    },
    store(self: BatchChallenge, b: c.Builder): void {
        b.storeUint(self.batchNo, 64);
        b.storeAddress(self.challenger);
        b.storeUint(self.reason, 8);
        b.storeUint(self.evidenceHash, 256);
        b.storeUint(self.openedAt, 32);
        b.storeUint(self.status, 8);
        b.storeCoins(self.challengerBond);
        b.storeCoins(self.slashedAmount);
    },
    toCell(self: BatchChallenge): c.Cell {
        return makeCellFrom<BatchChallenge>(self, BatchChallenge.store);
    }
}

/**
 > struct (0x4c324301) CommitBatch {
 >     batchNo: uint64
 >     rootsA: Cell<BatchRootsA>
 >     rootsB: Cell<BatchRootsB>
 > }
 */
export interface CommitBatch {
    readonly $: 'CommitBatch'
    batchNo: uint64
    rootsA: CellRef<BatchRootsA>
    rootsB: CellRef<BatchRootsB>
}

export const CommitBatch = {
    PREFIX: 0x4c324301,

    create(args: {
        batchNo: uint64
        rootsA: CellRef<BatchRootsA>
        rootsB: CellRef<BatchRootsB>
    }): CommitBatch {
        return {
            $: 'CommitBatch',
            ...args
        }
    },
    fromSlice(s: c.Slice): CommitBatch {
        loadAndCheckPrefix32(s, 0x4c324301, 'CommitBatch');
        return {
            $: 'CommitBatch',
            batchNo: s.loadUintBig(64),
            rootsA: loadCellRef<BatchRootsA>(s, BatchRootsA.fromSlice),
            rootsB: loadCellRef<BatchRootsB>(s, BatchRootsB.fromSlice),
        }
    },
    store(self: CommitBatch, b: c.Builder): void {
        b.storeUint(0x4c324301, 32);
        b.storeUint(self.batchNo, 64);
        storeCellRef<BatchRootsA>(self.rootsA, b, BatchRootsA.store);
        storeCellRef<BatchRootsB>(self.rootsB, b, BatchRootsB.store);
    },
    toCell(self: CommitBatch): c.Cell {
        return makeCellFrom<CommitBatch>(self, CommitBatch.store);
    }
}

/**
 > struct (0x4c324c09) SetAssetVault {
 >     assetVault: address
 > }
 */
export interface SetAssetVault {
    readonly $: 'SetAssetVault'
    assetVault: c.Address
}

export const SetAssetVault = {
    PREFIX: 0x4c324c09,

    create(args: {
        assetVault: c.Address
    }): SetAssetVault {
        return {
            $: 'SetAssetVault',
            ...args
        }
    },
    fromSlice(s: c.Slice): SetAssetVault {
        loadAndCheckPrefix32(s, 0x4c324c09, 'SetAssetVault');
        return {
            $: 'SetAssetVault',
            assetVault: s.loadAddress(),
        }
    },
    store(self: SetAssetVault, b: c.Builder): void {
        b.storeUint(0x4c324c09, 32);
        b.storeAddress(self.assetVault);
    },
    toCell(self: SetAssetVault): c.Cell {
        return makeCellFrom<SetAssetVault>(self, SetAssetVault.store);
    }
}

/**
 > struct (0x4c324602) FinalizeBatch {
 >     batchNo: uint64
 > }
 */
export interface FinalizeBatch {
    readonly $: 'FinalizeBatch'
    batchNo: uint64
}

export const FinalizeBatch = {
    PREFIX: 0x4c324602,

    create(args: {
        batchNo: uint64
    }): FinalizeBatch {
        return {
            $: 'FinalizeBatch',
            ...args
        }
    },
    fromSlice(s: c.Slice): FinalizeBatch {
        loadAndCheckPrefix32(s, 0x4c324602, 'FinalizeBatch');
        return {
            $: 'FinalizeBatch',
            batchNo: s.loadUintBig(64),
        }
    },
    store(self: FinalizeBatch, b: c.Builder): void {
        b.storeUint(0x4c324602, 32);
        b.storeUint(self.batchNo, 64);
    },
    toCell(self: FinalizeBatch): c.Cell {
        return makeCellFrom<FinalizeBatch>(self, FinalizeBatch.store);
    }
}

/**
 > struct (0x4c325704) ClaimWithdrawal {
 >     batchNo: uint64
 >     withdrawalId: uint256
 >     withdrawalLeaf: cell
 >     merkleProof: cell
 > }
 */
export interface ClaimWithdrawal {
    readonly $: 'ClaimWithdrawal'
    batchNo: uint64
    withdrawalId: uint256
    withdrawalLeaf: c.Cell
    merkleProof: c.Cell
}

export const ClaimWithdrawal = {
    PREFIX: 0x4c325704,

    create(args: {
        batchNo: uint64
        withdrawalId: uint256
        withdrawalLeaf: c.Cell
        merkleProof: c.Cell
    }): ClaimWithdrawal {
        return {
            $: 'ClaimWithdrawal',
            ...args
        }
    },
    fromSlice(s: c.Slice): ClaimWithdrawal {
        loadAndCheckPrefix32(s, 0x4c325704, 'ClaimWithdrawal');
        return {
            $: 'ClaimWithdrawal',
            batchNo: s.loadUintBig(64),
            withdrawalId: s.loadUintBig(256),
            withdrawalLeaf: s.loadRef(),
            merkleProof: s.loadRef(),
        }
    },
    store(self: ClaimWithdrawal, b: c.Builder): void {
        b.storeUint(0x4c325704, 32);
        b.storeUint(self.batchNo, 64);
        b.storeUint(self.withdrawalId, 256);
        b.storeRef(self.withdrawalLeaf);
        b.storeRef(self.merkleProof);
    },
    toCell(self: ClaimWithdrawal): c.Cell {
        return makeCellFrom<ClaimWithdrawal>(self, ClaimWithdrawal.store);
    }
}

/**
 > struct (0x4c325705) RetryWithdrawal {
 >     withdrawalId: uint256
 > }
 */
export interface RetryWithdrawal {
    readonly $: 'RetryWithdrawal'
    withdrawalId: uint256
}

export const RetryWithdrawal = {
    PREFIX: 0x4c325705,

    create(args: {
        withdrawalId: uint256
    }): RetryWithdrawal {
        return {
            $: 'RetryWithdrawal',
            ...args
        }
    },
    fromSlice(s: c.Slice): RetryWithdrawal {
        loadAndCheckPrefix32(s, 0x4c325705, 'RetryWithdrawal');
        return {
            $: 'RetryWithdrawal',
            withdrawalId: s.loadUintBig(256),
        }
    },
    store(self: RetryWithdrawal, b: c.Builder): void {
        b.storeUint(0x4c325705, 32);
        b.storeUint(self.withdrawalId, 256);
    },
    toCell(self: RetryWithdrawal): c.Cell {
        return makeCellFrom<RetryWithdrawal>(self, RetryWithdrawal.store);
    }
}

/**
 > struct (0x4c32530b) StakeSequencerBond {
 >     amount: coins
 > }
 */
export interface StakeSequencerBond {
    readonly $: 'StakeSequencerBond'
    amount: coins
}

export const StakeSequencerBond = {
    PREFIX: 0x4c32530b,

    create(args: {
        amount: coins
    }): StakeSequencerBond {
        return {
            $: 'StakeSequencerBond',
            ...args
        }
    },
    fromSlice(s: c.Slice): StakeSequencerBond {
        loadAndCheckPrefix32(s, 0x4c32530b, 'StakeSequencerBond');
        return {
            $: 'StakeSequencerBond',
            amount: s.loadCoins(),
        }
    },
    store(self: StakeSequencerBond, b: c.Builder): void {
        b.storeUint(0x4c32530b, 32);
        b.storeCoins(self.amount);
    },
    toCell(self: StakeSequencerBond): c.Cell {
        return makeCellFrom<StakeSequencerBond>(self, StakeSequencerBond.store);
    }
}

/**
 > struct (0x4c324348) ChallengeBatch {
 >     batchNo: uint64
 >     reason: uint8
 >     evidenceHash: uint256
 >     challengerBond: coins
 > }
 */
export interface ChallengeBatch {
    readonly $: 'ChallengeBatch'
    batchNo: uint64
    reason: uint8
    evidenceHash: uint256
    challengerBond: coins
}

export const ChallengeBatch = {
    PREFIX: 0x4c324348,

    create(args: {
        batchNo: uint64
        reason: uint8
        evidenceHash: uint256
        challengerBond: coins
    }): ChallengeBatch {
        return {
            $: 'ChallengeBatch',
            ...args
        }
    },
    fromSlice(s: c.Slice): ChallengeBatch {
        loadAndCheckPrefix32(s, 0x4c324348, 'ChallengeBatch');
        return {
            $: 'ChallengeBatch',
            batchNo: s.loadUintBig(64),
            reason: s.loadUintBig(8),
            evidenceHash: s.loadUintBig(256),
            challengerBond: s.loadCoins(),
        }
    },
    store(self: ChallengeBatch, b: c.Builder): void {
        b.storeUint(0x4c324348, 32);
        b.storeUint(self.batchNo, 64);
        b.storeUint(self.reason, 8);
        b.storeUint(self.evidenceHash, 256);
        b.storeCoins(self.challengerBond);
    },
    toCell(self: ChallengeBatch): c.Cell {
        return makeCellFrom<ChallengeBatch>(self, ChallengeBatch.store);
    }
}

/**
 > struct (0x4c325243) ResolveChallenge {
 >     batchNo: uint64
 >     uphold: bool
 >     slashAmount: coins
 > }
 */
export interface ResolveChallenge {
    readonly $: 'ResolveChallenge'
    batchNo: uint64
    uphold: boolean
    slashAmount: coins
}

export const ResolveChallenge = {
    PREFIX: 0x4c325243,

    create(args: {
        batchNo: uint64
        uphold: boolean
        slashAmount: coins
    }): ResolveChallenge {
        return {
            $: 'ResolveChallenge',
            ...args
        }
    },
    fromSlice(s: c.Slice): ResolveChallenge {
        loadAndCheckPrefix32(s, 0x4c325243, 'ResolveChallenge');
        return {
            $: 'ResolveChallenge',
            batchNo: s.loadUintBig(64),
            uphold: s.loadBoolean(),
            slashAmount: s.loadCoins(),
        }
    },
    store(self: ResolveChallenge, b: c.Builder): void {
        b.storeUint(0x4c325243, 32);
        b.storeUint(self.batchNo, 64);
        b.storeBit(self.uphold);
        b.storeCoins(self.slashAmount);
    },
    toCell(self: ResolveChallenge): c.Cell {
        return makeCellFrom<ResolveChallenge>(self, ResolveChallenge.store);
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
 > struct RollupStorage {
 >     admin: address
 >     sequencer: address
 >     assetVault: address
 >     challengeWindowSec: uint32
 >     lastCommitted: uint64
 >     lastFinalized: uint64
 >     paused: bool
 >     security: Cell<RollupSecurityState>
 >     commitments: map<uint64, Cell<BatchCommitment>>
 >     claimedWithdrawals: map<uint256, bool>
 >     failedWithdrawals: map<uint256, Cell<ReleaseFailure>>
 > }
 */
export interface RollupStorage {
    readonly $: 'RollupStorage'
    admin: c.Address
    sequencer: c.Address
    assetVault: c.Address
    challengeWindowSec: uint32
    lastCommitted: uint64
    lastFinalized: uint64
    paused: boolean
    security: CellRef<RollupSecurityState>
    commitments: c.Dictionary<uint64, CellRef<BatchCommitment>>
    claimedWithdrawals: c.Dictionary<uint256, boolean>
    failedWithdrawals: c.Dictionary<uint256, CellRef<ReleaseFailure>>
}

export const RollupStorage = {
    create(args: {
        admin: c.Address
        sequencer: c.Address
        assetVault: c.Address
        challengeWindowSec: uint32
        lastCommitted: uint64
        lastFinalized: uint64
        paused: boolean
        security: CellRef<RollupSecurityState>
        commitments: c.Dictionary<uint64, CellRef<BatchCommitment>>
        claimedWithdrawals: c.Dictionary<uint256, boolean>
        failedWithdrawals: c.Dictionary<uint256, CellRef<ReleaseFailure>>
    }): RollupStorage {
        return {
            $: 'RollupStorage',
            ...args
        }
    },
    fromSlice(s: c.Slice): RollupStorage {
        return {
            $: 'RollupStorage',
            admin: s.loadAddress(),
            sequencer: s.loadAddress(),
            assetVault: s.loadAddress(),
            challengeWindowSec: s.loadUintBig(32),
            lastCommitted: s.loadUintBig(64),
            lastFinalized: s.loadUintBig(64),
            paused: s.loadBoolean(),
            security: loadCellRef<RollupSecurityState>(s, RollupSecurityState.fromSlice),
            commitments: c.Dictionary.load<uint64, CellRef<BatchCommitment>>(c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<BatchCommitment>>(
                (s) => loadCellRef<BatchCommitment>(s, BatchCommitment.fromSlice),
                (v,b) => storeCellRef<BatchCommitment>(v, b, BatchCommitment.store)
            ), s),
            claimedWithdrawals: c.Dictionary.load<uint256, boolean>(c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.Bool(), s),
            failedWithdrawals: c.Dictionary.load<uint256, CellRef<ReleaseFailure>>(c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailure>>(
                (s) => loadCellRef<ReleaseFailure>(s, ReleaseFailure.fromSlice),
                (v,b) => storeCellRef<ReleaseFailure>(v, b, ReleaseFailure.store)
            ), s),
        }
    },
    store(self: RollupStorage, b: c.Builder): void {
        b.storeAddress(self.admin);
        b.storeAddress(self.sequencer);
        b.storeAddress(self.assetVault);
        b.storeUint(self.challengeWindowSec, 32);
        b.storeUint(self.lastCommitted, 64);
        b.storeUint(self.lastFinalized, 64);
        b.storeBit(self.paused);
        storeCellRef<RollupSecurityState>(self.security, b, RollupSecurityState.store);
        b.storeDict<uint64, CellRef<BatchCommitment>>(self.commitments, c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<BatchCommitment>>(
            (s) => loadCellRef<BatchCommitment>(s, BatchCommitment.fromSlice),
            (v,b) => storeCellRef<BatchCommitment>(v, b, BatchCommitment.store)
        ));
        b.storeDict<uint256, boolean>(self.claimedWithdrawals, c.Dictionary.Keys.BigUint(256), c.Dictionary.Values.Bool());
        b.storeDict<uint256, CellRef<ReleaseFailure>>(self.failedWithdrawals, c.Dictionary.Keys.BigUint(256), createDictionaryValue<CellRef<ReleaseFailure>>(
            (s) => loadCellRef<ReleaseFailure>(s, ReleaseFailure.fromSlice),
            (v,b) => storeCellRef<ReleaseFailure>(v, b, ReleaseFailure.store)
        ));
    },
    toCell(self: RollupStorage): c.Cell {
        return makeCellFrom<RollupStorage>(self, RollupStorage.store);
    }
}

/**
 > struct RollupSecurityState {
 >     sequencerBond: coins
 >     slashedBond: coins
 >     challenges: map<uint64, Cell<BatchChallenge>>
 > }
 */
export interface RollupSecurityState {
    readonly $: 'RollupSecurityState'
    sequencerBond: coins
    slashedBond: coins
    challenges: c.Dictionary<uint64, CellRef<BatchChallenge>>
}

export const RollupSecurityState = {
    create(args: {
        sequencerBond: coins
        slashedBond: coins
        challenges: c.Dictionary<uint64, CellRef<BatchChallenge>>
    }): RollupSecurityState {
        return {
            $: 'RollupSecurityState',
            ...args
        }
    },
    fromSlice(s: c.Slice): RollupSecurityState {
        return {
            $: 'RollupSecurityState',
            sequencerBond: s.loadCoins(),
            slashedBond: s.loadCoins(),
            challenges: c.Dictionary.load<uint64, CellRef<BatchChallenge>>(c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<BatchChallenge>>(
                (s) => loadCellRef<BatchChallenge>(s, BatchChallenge.fromSlice),
                (v,b) => storeCellRef<BatchChallenge>(v, b, BatchChallenge.store)
            ), s),
        }
    },
    store(self: RollupSecurityState, b: c.Builder): void {
        b.storeCoins(self.sequencerBond);
        b.storeCoins(self.slashedBond);
        b.storeDict<uint64, CellRef<BatchChallenge>>(self.challenges, c.Dictionary.Keys.BigUint(64), createDictionaryValue<CellRef<BatchChallenge>>(
            (s) => loadCellRef<BatchChallenge>(s, BatchChallenge.fromSlice),
            (v,b) => storeCellRef<BatchChallenge>(v, b, BatchChallenge.store)
        ));
    },
    toCell(self: RollupSecurityState): c.Cell {
        return makeCellFrom<RollupSecurityState>(self, RollupSecurityState.store);
    }
}

/**
 > struct RollupStatusReply {
 >     sequencer: address
 >     assetVault: address
 >     challengeWindowSec: uint32
 >     lastCommitted: uint64
 >     lastFinalized: uint64
 >     paused: bool
 > }
 */
export interface RollupStatusReply {
    readonly $: 'RollupStatusReply'
    sequencer: c.Address
    assetVault: c.Address
    challengeWindowSec: uint32
    lastCommitted: uint64
    lastFinalized: uint64
    paused: boolean
}

export const RollupStatusReply = {
    create(args: {
        sequencer: c.Address
        assetVault: c.Address
        challengeWindowSec: uint32
        lastCommitted: uint64
        lastFinalized: uint64
        paused: boolean
    }): RollupStatusReply {
        return {
            $: 'RollupStatusReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): RollupStatusReply {
        return {
            $: 'RollupStatusReply',
            sequencer: s.loadAddress(),
            assetVault: s.loadAddress(),
            challengeWindowSec: s.loadUintBig(32),
            lastCommitted: s.loadUintBig(64),
            lastFinalized: s.loadUintBig(64),
            paused: s.loadBoolean(),
        }
    },
    store(self: RollupStatusReply, b: c.Builder): void {
        b.storeAddress(self.sequencer);
        b.storeAddress(self.assetVault);
        b.storeUint(self.challengeWindowSec, 32);
        b.storeUint(self.lastCommitted, 64);
        b.storeUint(self.lastFinalized, 64);
        b.storeBit(self.paused);
    },
    toCell(self: RollupStatusReply): c.Cell {
        return makeCellFrom<RollupStatusReply>(self, RollupStatusReply.store);
    }
}

/**
 > struct CommitmentReply {
 >     exists: bool
 >     commitment: Cell<BatchCommitment>
 > }
 */
export interface CommitmentReply {
    readonly $: 'CommitmentReply'
    exists: boolean
    commitment: CellRef<BatchCommitment>
}

export const CommitmentReply = {
    create(args: {
        exists: boolean
        commitment: CellRef<BatchCommitment>
    }): CommitmentReply {
        return {
            $: 'CommitmentReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): CommitmentReply {
        return {
            $: 'CommitmentReply',
            exists: s.loadBoolean(),
            commitment: loadCellRef<BatchCommitment>(s, BatchCommitment.fromSlice),
        }
    },
    store(self: CommitmentReply, b: c.Builder): void {
        b.storeBit(self.exists);
        storeCellRef<BatchCommitment>(self.commitment, b, BatchCommitment.store);
    },
    toCell(self: CommitmentReply): c.Cell {
        return makeCellFrom<CommitmentReply>(self, CommitmentReply.store);
    }
}

/**
 > struct BondStatusReply {
 >     sequencer: address
 >     sequencerBond: coins
 >     slashedBond: coins
 > }
 */
export interface BondStatusReply {
    readonly $: 'BondStatusReply'
    sequencer: c.Address
    sequencerBond: coins
    slashedBond: coins
}

export const BondStatusReply = {
    create(args: {
        sequencer: c.Address
        sequencerBond: coins
        slashedBond: coins
    }): BondStatusReply {
        return {
            $: 'BondStatusReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): BondStatusReply {
        return {
            $: 'BondStatusReply',
            sequencer: s.loadAddress(),
            sequencerBond: s.loadCoins(),
            slashedBond: s.loadCoins(),
        }
    },
    store(self: BondStatusReply, b: c.Builder): void {
        b.storeAddress(self.sequencer);
        b.storeCoins(self.sequencerBond);
        b.storeCoins(self.slashedBond);
    },
    toCell(self: BondStatusReply): c.Cell {
        return makeCellFrom<BondStatusReply>(self, BondStatusReply.store);
    }
}

/**
 > struct ChallengeReply {
 >     exists: bool
 >     challenge: Cell<BatchChallenge>
 > }
 */
export interface ChallengeReply {
    readonly $: 'ChallengeReply'
    exists: boolean
    challenge: CellRef<BatchChallenge>
}

export const ChallengeReply = {
    create(args: {
        exists: boolean
        challenge: CellRef<BatchChallenge>
    }): ChallengeReply {
        return {
            $: 'ChallengeReply',
            ...args
        }
    },
    fromSlice(s: c.Slice): ChallengeReply {
        return {
            $: 'ChallengeReply',
            exists: s.loadBoolean(),
            challenge: loadCellRef<BatchChallenge>(s, BatchChallenge.fromSlice),
        }
    },
    store(self: ChallengeReply, b: c.Builder): void {
        b.storeBit(self.exists);
        storeCellRef<BatchChallenge>(self.challenge, b, BatchChallenge.store);
    },
    toCell(self: ChallengeReply): c.Cell {
        return makeCellFrom<ChallengeReply>(self, ChallengeReply.store);
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

// ————————————————————————————————————————————
//    class RollupRoot
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

export class RollupRoot implements c.Contract {
    static CodeCell = c.Cell.fromBase64('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');

    static Errors = {
        'Errors.Unauthorized': 4097,
        'Errors.Paused': 4098,
        'Errors.BadBatchNo': 4099,
        'Errors.BadPrevRoot': 4100,
        'Errors.BatchNotFound': 4101,
        'Errors.ChallengeWindow': 4102,
        'Errors.AlreadyFinalized': 4103,
        'Errors.NotFinalized': 4104,
        'Errors.AlreadyClaimed': 4105,
        'Errors.BadWithdrawalProof': 4106,
        'Errors.NoFailedWithdrawal': 4109,
        'Errors.BadAssetConfig': 4115,
        'Errors.AlreadyConfigured': 4116,
        'Errors.BadChallenge': 4118,
        'Errors.ChallengeOpen': 4119,
        'Errors.ChallengeNotFound': 4120,
        'Errors.InsufficientBond': 4121,
        'Errors.ChallengeAlreadyExists': 4122,
        'Errors.UnknownOpcode': 65535,
    }

    readonly address: c.Address
    readonly init: { code: c.Cell, data: c.Cell } | undefined

    protected constructor(address: c.Address, init?: { code: c.Cell, data: c.Cell }) {
        this.address = address;
        this.init = init;
    }

    static fromAddress(address: c.Address) {
        return new RollupRoot(address);
    }

    static fromStorage(emptyStorage: {
        admin: c.Address
        sequencer: c.Address
        assetVault: c.Address
        challengeWindowSec: uint32
        lastCommitted: uint64
        lastFinalized: uint64
        paused: boolean
        security: CellRef<RollupSecurityState>
        commitments: c.Dictionary<uint64, CellRef<BatchCommitment>>
        claimedWithdrawals: c.Dictionary<uint256, boolean>
        failedWithdrawals: c.Dictionary<uint256, CellRef<ReleaseFailure>>
    }, deployedOptions?: DeployedAddrOptions) {
        const initialState = {
            code: deployedOptions?.overrideContractCode ?? RollupRoot.CodeCell,
            data: RollupStorage.toCell(RollupStorage.create(emptyStorage)),
        };
        const address = calculateDeployedAddress(initialState.code, initialState.data, deployedOptions ?? {});
        return new RollupRoot(address, initialState);
    }

    static createCellOfCommitBatch(body: {
        batchNo: uint64
        rootsA: CellRef<BatchRootsA>
        rootsB: CellRef<BatchRootsB>
    }) {
        return CommitBatch.toCell(CommitBatch.create(body));
    }

    static createCellOfSetAssetVault(body: {
        assetVault: c.Address
    }) {
        return SetAssetVault.toCell(SetAssetVault.create(body));
    }

    static createCellOfFinalizeBatch(body: {
        batchNo: uint64
    }) {
        return FinalizeBatch.toCell(FinalizeBatch.create(body));
    }

    static createCellOfClaimWithdrawal(body: {
        batchNo: uint64
        withdrawalId: uint256
        withdrawalLeaf: c.Cell
        merkleProof: c.Cell
    }) {
        return ClaimWithdrawal.toCell(ClaimWithdrawal.create(body));
    }

    static createCellOfRetryWithdrawal(body: {
        withdrawalId: uint256
    }) {
        return RetryWithdrawal.toCell(RetryWithdrawal.create(body));
    }

    static createCellOfStakeSequencerBond(body: {
        amount: coins
    }) {
        return StakeSequencerBond.toCell(StakeSequencerBond.create(body));
    }

    static createCellOfChallengeBatch(body: {
        batchNo: uint64
        reason: uint8
        evidenceHash: uint256
        challengerBond: coins
    }) {
        return ChallengeBatch.toCell(ChallengeBatch.create(body));
    }

    static createCellOfResolveChallenge(body: {
        batchNo: uint64
        uphold: boolean
        slashAmount: coins
    }) {
        return ResolveChallenge.toCell(ResolveChallenge.create(body));
    }

    async sendDeploy(provider: ContractProvider, via: Sender, msgValue: coins, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: c.Cell.EMPTY,
            ...extraOptions
        });
    }

    async sendCommitBatch(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        batchNo: uint64
        rootsA: CellRef<BatchRootsA>
        rootsB: CellRef<BatchRootsB>
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: CommitBatch.toCell(CommitBatch.create(body)),
            ...extraOptions
        });
    }

    async sendSetAssetVault(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        assetVault: c.Address
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: SetAssetVault.toCell(SetAssetVault.create(body)),
            ...extraOptions
        });
    }

    async sendFinalizeBatch(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        batchNo: uint64
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: FinalizeBatch.toCell(FinalizeBatch.create(body)),
            ...extraOptions
        });
    }

    async sendClaimWithdrawal(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        batchNo: uint64
        withdrawalId: uint256
        withdrawalLeaf: c.Cell
        merkleProof: c.Cell
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ClaimWithdrawal.toCell(ClaimWithdrawal.create(body)),
            ...extraOptions
        });
    }

    async sendRetryWithdrawal(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        withdrawalId: uint256
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: RetryWithdrawal.toCell(RetryWithdrawal.create(body)),
            ...extraOptions
        });
    }

    async sendStakeSequencerBond(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        amount: coins
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: StakeSequencerBond.toCell(StakeSequencerBond.create(body)),
            ...extraOptions
        });
    }

    async sendChallengeBatch(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        batchNo: uint64
        reason: uint8
        evidenceHash: uint256
        challengerBond: coins
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ChallengeBatch.toCell(ChallengeBatch.create(body)),
            ...extraOptions
        });
    }

    async sendResolveChallenge(provider: ContractProvider, via: Sender, msgValue: coins, body: {
        batchNo: uint64
        uphold: boolean
        slashAmount: coins
    }, extraOptions?: ExtraSendOptions) {
        return provider.internal(via, {
            value: msgValue,
            body: ResolveChallenge.toCell(ResolveChallenge.create(body)),
            ...extraOptions
        });
    }

    async getRollupStatus(provider: ContractProvider): Promise<RollupStatusReply> {
        const r = StackReader.fromGetMethod(6, await provider.get('rollupStatus', []));
        return ({
            $: 'RollupStatusReply',
            sequencer: r.readSlice().loadAddress(),
            assetVault: r.readSlice().loadAddress(),
            challengeWindowSec: r.readBigInt(),
            lastCommitted: r.readBigInt(),
            lastFinalized: r.readBigInt(),
            paused: r.readBoolean(),
        });
    }

    async getCommitment(provider: ContractProvider, batchNo: uint64): Promise<CommitmentReply> {
        const r = StackReader.fromGetMethod(2, await provider.get('commitment', [
            { type: 'int', value: batchNo },
        ]));
        return ({
            $: 'CommitmentReply',
            exists: r.readBoolean(),
            commitment: r.readCellRef<BatchCommitment>(BatchCommitment.fromSlice),
        });
    }

    async getBondStatus(provider: ContractProvider): Promise<BondStatusReply> {
        const r = StackReader.fromGetMethod(3, await provider.get('bondStatus', []));
        return ({
            $: 'BondStatusReply',
            sequencer: r.readSlice().loadAddress(),
            sequencerBond: r.readBigInt(),
            slashedBond: r.readBigInt(),
        });
    }

    async getBatchChallenge(provider: ContractProvider, batchNo: uint64): Promise<ChallengeReply> {
        const r = StackReader.fromGetMethod(2, await provider.get('batchChallenge', [
            { type: 'int', value: batchNo },
        ]));
        return ({
            $: 'ChallengeReply',
            exists: r.readBoolean(),
            challenge: r.readCellRef<BatchChallenge>(BatchChallenge.fromSlice),
        });
    }

    async getFailedWithdrawal(provider: ContractProvider, withdrawalId: uint256): Promise<ReleaseFailureReply> {
        const r = StackReader.fromGetMethod(2, await provider.get('failedWithdrawal', [
            { type: 'int', value: withdrawalId },
        ]));
        return ({
            $: 'ReleaseFailureReply',
            exists: r.readBoolean(),
            failure: r.readCellRef<ReleaseFailure>(ReleaseFailure.fromSlice),
        });
    }
}
