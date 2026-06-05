import { createHash } from "node:crypto";
import type { Hash32, SignedL2Transaction } from "./index.js";

export const CONSENSUS_ENCODING_VERSION = 1;
export const L2_TX_VERSION_V2 = 2;
export const L2_TRANSACTION_KIND_VERSION_V1 = 1;
export const L2_TX_DOMAIN_SEPARATOR = "entropis.l2.tx.v2";

const MAGIC = Buffer.from("EL2C");
const TYPE_UNSIGNED_TX = 0x01;
const TYPE_RECEIPT = 0x02;
const TYPE_WITHDRAWAL_LEAF = 0x03;
const TYPE_ACCOUNT_LEAF = 0x04;
const TYPE_BLOCK_HEADER = 0x05;
const TYPE_BATCH_DATA = 0x06;
const TYPE_SIGNED_TX = 0x07;

const KIND_DEPOSIT = 0x01;
const KIND_TRANSFER = 0x02;
const KIND_WITHDRAW = 0x03;
const KIND_CALL_CONTRACT = 0x04;
const KIND_DEPLOY_CONTRACT = 0x05;
const KIND_ROTATE_PUBLIC_KEY = 0x06;

const STATUS_APPLIED = 0x01;
const STATUS_REJECTED = 0x02;

export type ReceiptStatus = "Applied" | "Rejected";

export interface Receipt {
  tx_hash: Hash32;
  status: ReceiptStatus;
  gas_charged: string;
  reason: string | null;
  withdrawal_id: Hash32 | null;
}

export interface WithdrawalLeaf {
  withdrawal_id: Hash32;
  asset_id: number;
  amount: string;
  l2_sender: Hash32;
  l1_recipient: string;
}

export interface L2BlockHeader {
  height: number | bigint;
  prev_block_hash: Hash32;
  prev_state_root: Hash32;
  state_root: Hash32;
  tx_root: Hash32;
  receipt_root: Hash32;
  withdrawal_root: Hash32;
  data_hash: Hash32;
  timestamp: number | bigint;
}

export type AccountType = "user" | "contract" | "system" | "operator";

export interface AccountFlags {
  disabled?: boolean;
  contract_only?: boolean;
  system_only?: boolean;
}

export interface AccountRecoveryLock {
  locked: boolean;
  admin: Hash32 | null;
}

export interface AccountLeaf {
  account_type?: AccountType;
  flags?: AccountFlags;
  active_public_key?: Hash32 | null;
  recovery_lock?: AccountRecoveryLock | null;
  nonce: number | bigint;
  balances: Array<{ asset_id: number; balance: string | number | bigint }>;
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
  last_lt: number | bigint;
}

export function sha256Hex(data: Uint8Array | string): Hash32 {
  return createHash("sha256").update(data).digest("hex");
}

export function hashDomain(domain: string, parts: Uint8Array[]): Hash32 {
  const hash = createHash("sha256");
  const domainBytes = Buffer.from(domain);
  hash.update(u64be(BigInt(domainBytes.length)));
  hash.update(domainBytes);
  for (const part of parts) {
    hash.update(u64be(BigInt(part.length)));
    hash.update(part);
  }
  return hash.digest("hex");
}

export function deriveAccountId(publicKey: Uint8Array): Hash32 {
  if (publicKey.length !== 32) {
    throw new Error("ed25519 public key must be 32 bytes");
  }
  return hashDomain("l2.account.ed25519.v1", [publicKey]);
}

export function encodeUnsignedTransaction(tx: SignedL2Transaction): Uint8Array {
  const out = writer(TYPE_UNSIGNED_TX);
  writeUnsignedTransactionBody(out, tx);
  return out.bytes();
}

export function encodeSignedTransaction(tx: SignedL2Transaction): Uint8Array {
  const out = writer(TYPE_SIGNED_TX);
  writeUnsignedTransactionBody(out, tx);
  writeOptionalString(out, tx.public_key);
  writeOptionalString(out, tx.signature);
  return out.bytes();
}

export function signingPayload(tx: SignedL2Transaction): Uint8Array {
  return encodeUnsignedTransaction(tx);
}

export function txHash(tx: SignedL2Transaction): Hash32 {
  return hashDomain("l2.tx.v2", [signingPayload(tx)]);
}

export function encodeReceipt(receipt: Receipt): Uint8Array {
  const out = writer(TYPE_RECEIPT);
  out.hash(receipt.tx_hash);
  out.u8(receipt.status === "Applied" ? STATUS_APPLIED : STATUS_REJECTED);
  out.u128(receipt.gas_charged);
  writeOptionalString(out, receipt.reason);
  writeOptionalHash(out, receipt.withdrawal_id);
  return out.bytes();
}

export function receiptLeafHash(receipt: Receipt): Hash32 {
  return hashDomain("l2.receipt.leaf.v1", [encodeReceipt(receipt)]);
}

export function withdrawalId(
  tx_hash: Hash32,
  asset_id: number,
  amount: string | number | bigint,
  l2_sender: Hash32,
  l1_recipient: string,
): Hash32 {
  const out = rawWriter();
  out.hash(tx_hash);
  out.u32(asset_id);
  out.u128(amount);
  out.hash(l2_sender);
  out.string(l1_recipient);
  return hashDomain("l2.withdrawal.id.v1", [out.bytes()]);
}

export function encodeWithdrawalLeaf(leaf: WithdrawalLeaf): Uint8Array {
  const out = writer(TYPE_WITHDRAWAL_LEAF);
  out.hash(leaf.withdrawal_id);
  out.u32(leaf.asset_id);
  out.u128(leaf.amount);
  out.hash(leaf.l2_sender);
  out.string(leaf.l1_recipient);
  return out.bytes();
}

export function withdrawalLeafHash(leaf: WithdrawalLeaf): Hash32 {
  return hashDomain("l2.withdrawal.leaf.v1", [encodeWithdrawalLeaf(leaf)]);
}

export function encodeAccountLeaf(account_id: Hash32, account: AccountLeaf): Uint8Array {
  const out = writer(TYPE_ACCOUNT_LEAF);
  out.hash(account_id);
  out.u8(accountTypeTag(account.account_type ?? "user"));
  out.u8(accountFlagsBits(account.flags ?? {}));
  writeOptionalHash(out, account.active_public_key ?? null);
  writeOptionalRecoveryLock(out, account.recovery_lock ?? null);
  out.u64(account.nonce);
  const balances = [...account.balances].sort((a, b) => a.asset_id - b.asset_id);
  out.len(balances.length);
  let previousAssetId: number | null = null;
  for (const balance of balances) {
    if (balance.asset_id === previousAssetId) {
      throw new Error("duplicate account balance asset id");
    }
    out.u32(balance.asset_id);
    out.u128(balance.balance);
    previousAssetId = balance.asset_id;
  }
  out.hash(account.code_hash);
  out.hash(account.data_hash);
  out.hash(account.storage_root);
  out.u64(account.last_lt);
  return out.bytes();
}

export function accountLeafHash(account_id: Hash32, account: AccountLeaf): Hash32 {
  return hashDomain("l2.state.account.v1", [encodeAccountLeaf(account_id, account)]);
}

export function encodeBlockHeader(header: L2BlockHeader): Uint8Array {
  const out = writer(TYPE_BLOCK_HEADER);
  out.u64(header.height);
  out.hash(header.prev_block_hash);
  out.hash(header.prev_state_root);
  out.hash(header.state_root);
  out.hash(header.tx_root);
  out.hash(header.receipt_root);
  out.hash(header.withdrawal_root);
  out.hash(header.data_hash);
  out.u64(header.timestamp);
  return out.bytes();
}

export function blockHeaderHash(header: L2BlockHeader): Hash32 {
  return hashDomain("l2.block.header.v1", [encodeBlockHeader(header)]);
}

export function encodeBatchData(txs: SignedL2Transaction[], receipts: Receipt[]): Uint8Array {
  const out = writer(TYPE_BATCH_DATA);
  out.len(txs.length);
  for (const tx of txs) {
    out.bytesWithLength(encodeSignedTransaction(tx));
  }
  out.len(receipts.length);
  for (const receipt of receipts) {
    out.bytesWithLength(encodeReceipt(receipt));
  }
  return out.bytes();
}

export function canonicalBatchDataHash(
  txs: SignedL2Transaction[],
  receipts: Receipt[],
): Hash32 {
  return hashDomain("l2.batch.data.v1", [encodeBatchData(txs, receipts)]);
}

function writeUnsignedTransactionBody(out: ConsensusWriter, tx: SignedL2Transaction) {
  out.u16(tx.tx_version);
  out.string(tx.domain_separator);
  out.string(tx.chain_id);
  writeOptionalHash(out, tx.from);
  out.u64(tx.nonce);
  out.u64(tx.valid_until_block);
  out.u64(tx.gas_limit);
  out.u128(tx.max_gas_price);
  out.u32(tx.fee_asset_id);
  writeOptionalHash(out, tx.memo_hash);
  out.u16(tx.transaction_kind_version);

  if ("Deposit" in tx.kind) {
    out.u8(KIND_DEPOSIT);
    out.hash(tx.kind.Deposit.deposit_id);
    out.u32(tx.kind.Deposit.asset_id);
    out.hash(tx.kind.Deposit.recipient);
    out.u128(tx.kind.Deposit.amount);
  } else if ("Transfer" in tx.kind) {
    out.u8(KIND_TRANSFER);
    out.hash(tx.kind.Transfer.to);
    out.u32(tx.kind.Transfer.asset_id);
    out.u128(tx.kind.Transfer.amount);
  } else if ("RotatePublicKey" in tx.kind) {
    out.u8(KIND_ROTATE_PUBLIC_KEY);
    out.string(tx.kind.RotatePublicKey.new_public_key);
  } else if ("Withdraw" in tx.kind) {
    out.u8(KIND_WITHDRAW);
    out.u32(tx.kind.Withdraw.asset_id);
    out.u128(tx.kind.Withdraw.amount);
    out.string(tx.kind.Withdraw.l1_recipient);
  } else if ("CallContract" in tx.kind) {
    out.u8(KIND_CALL_CONTRACT);
    out.hash(tx.kind.CallContract.contract);
    out.string(tx.kind.CallContract.body_boc_base64);
  } else {
    out.u8(KIND_DEPLOY_CONTRACT);
    out.hash(tx.kind.DeployContract.contract);
    out.string(tx.kind.DeployContract.code_boc_base64);
    out.string(tx.kind.DeployContract.data_boc_base64);
  }
}

function accountTypeTag(accountType: AccountType): number {
  switch (accountType) {
    case "user":
      return 1;
    case "contract":
      return 2;
    case "system":
      return 3;
    case "operator":
      return 4;
    default:
      throw new Error("unknown account type");
  }
}

function accountFlagsBits(flags: AccountFlags): number {
  let bits = 0;
  if (flags.disabled) {
    bits |= 1 << 0;
  }
  if (flags.contract_only) {
    bits |= 1 << 1;
  }
  if (flags.system_only) {
    bits |= 1 << 2;
  }
  return bits;
}

function writeOptionalHash(out: ConsensusWriter, value: Hash32 | null) {
  if (value !== null) {
    out.u8(1);
    out.hash(value);
  } else {
    out.u8(0);
  }
}

function writeOptionalString(out: ConsensusWriter, value: string | null) {
  if (value !== null) {
    out.u8(1);
    out.string(value);
  } else {
    out.u8(0);
  }
}

function writeOptionalRecoveryLock(out: ConsensusWriter, value: AccountRecoveryLock | null) {
  if (value !== null) {
    out.u8(1);
    out.u8(value.locked ? 1 : 0);
    writeOptionalHash(out, value.admin);
  } else {
    out.u8(0);
  }
}

function writer(typeTag: number): ConsensusWriter {
  const out = rawWriter();
  out.raw(MAGIC);
  out.u8(CONSENSUS_ENCODING_VERSION);
  out.u8(typeTag);
  return out;
}

function rawWriter(): ConsensusWriter {
  return new ConsensusWriter();
}

class ConsensusWriter {
  private chunks: Buffer[] = [];

  raw(value: Uint8Array) {
    this.chunks.push(Buffer.from(value));
  }

  u8(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xff) {
      throw new Error("expected uint8");
    }
    this.chunks.push(Buffer.from([value & 0xff]));
  }

  u32(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
      throw new Error("expected uint32");
    }
    const out = Buffer.alloc(4);
    out.writeUInt32BE(value);
    this.chunks.push(out);
  }

  u16(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
      throw new Error("expected uint16");
    }
    const out = Buffer.alloc(2);
    out.writeUInt16BE(value);
    this.chunks.push(out);
  }

  u64(value: number | bigint) {
    const out = Buffer.alloc(8);
    out.writeBigUInt64BE(toUint(value, 64));
    this.chunks.push(out);
  }

  u128(value: string | number | bigint) {
    const bigint = toUint(value, 128);
    const out = Buffer.alloc(16);
    out.writeBigUInt64BE(bigint >> 64n, 0);
    out.writeBigUInt64BE(bigint & 0xffffffffffffffffn, 8);
    this.chunks.push(out);
  }

  hash(value: Hash32) {
    this.raw(hexToHashBytes(value));
  }

  string(value: string) {
    this.bytesWithLength(Buffer.from(value, "utf8"));
  }

  bytesWithLength(value: Uint8Array) {
    this.len(value.length);
    this.raw(value);
  }

  len(value: number) {
    this.u32(value);
  }

  bytes(): Buffer {
    return Buffer.concat(this.chunks);
  }
}

function hexToHashBytes(value: string): Buffer {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return Buffer.from(cleaned, "hex");
}

function toUint(value: string | number | bigint, bits: number): bigint {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`expected safe uint${bits} number`);
  }
  const bigint = typeof value === "bigint" ? value : BigInt(value);
  if (bigint < 0n || bigint >= (1n << BigInt(bits))) {
    throw new Error(`expected uint${bits}`);
  }
  return bigint;
}

function u64be(value: bigint): Buffer {
  const out = Buffer.alloc(8);
  out.writeBigUInt64BE(value);
  return out;
}
