import { beginCell, Cell } from "@ton/core";
import nacl from "tweetnacl";
import { hashDomain, signingPayload } from "./consensus.js";

export type Hash32 = string;
type UIntLike = bigint | number | string;

export const SAMPLE_COUNTER_INCREMENT_OPCODE = 0x534c3201;
export const SAMPLE_COUNTER_INCREMENT_GAS = 25;
const SAMPLE_COUNTER_STORAGE_PREFIX = Buffer.from("L2CNTR01", "ascii");

export interface SignedL2Transaction {
  chain_id: string;
  from: Hash32 | null;
  nonce: number;
  gas_limit: number;
  max_gas_price: string;
  kind:
    | {
        DeployContract: {
          contract: Hash32;
          code_hash: Hash32;
          data_hash: Hash32;
          storage_root: Hash32;
        };
      }
    | { CallContract: { contract: Hash32; body_boc_base64: string } };
  public_key: string | null;
  signature: string | null;
}

export interface SigningParams {
  keyPair: nacl.SignKeyPair;
}

export interface L2Account {
  nonce: number;
  balances: Record<string, string | number>;
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
  last_lt: number;
}

export interface DeployContractTransactionParams {
  chainId: string;
  from: Hash32;
  nonce: UIntLike;
  contract: Hash32;
  codeHash: Hash32;
  dataHash: Hash32;
  storageRoot: Hash32;
  gasLimit: UIntLike;
  maxGasPrice: UIntLike;
}

export interface CallContractTransactionParams {
  chainId: string;
  from: Hash32;
  nonce: UIntLike;
  contract: Hash32;
  bodyBocBase64: string;
  gasLimit: UIntLike;
  maxGasPrice: UIntLike;
}

export interface SampleCounterState {
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
}

export interface SampleCounterReadResponse extends SampleCounterState {
  contract: Hash32;
  counter: number;
}

export function buildDeployContractTransaction(
  params: DeployContractTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  const contract = normalizeHash32(params.contract);
  const codeHash = normalizeHash32(params.codeHash);
  const dataHash = normalizeHash32(params.dataHash);
  const storageRoot = normalizeHash32(params.storageRoot);
  if (contract === zeroHash32() || codeHash === zeroHash32() || dataHash === zeroHash32()) {
    throw new Error("contract, codeHash, and dataHash must be non-zero");
  }
  if (storageRoot === zeroHash32()) {
    throw new Error("storageRoot must be non-zero");
  }
  return {
    chain_id: params.chainId,
    from: normalizeHash32(params.from),
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      DeployContract: {
        contract,
        code_hash: codeHash,
        data_hash: dataHash,
        storage_root: storageRoot,
      },
    },
  };
}

export function signDeployContractTransaction(
  params: DeployContractTransactionParams & SigningParams,
): SignedL2Transaction {
  return signTransaction(buildDeployContractTransaction(params), params.keyPair);
}

export function buildCallContractTransaction(
  params: CallContractTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  normalizeSingleRootBocBase64(params.bodyBocBase64, "bodyBocBase64");
  return {
    chain_id: params.chainId,
    from: normalizeHash32(params.from),
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      CallContract: {
        contract: normalizeHash32(params.contract),
        body_boc_base64: params.bodyBocBase64,
      },
    },
  };
}

export function signCallContractTransaction(
  params: CallContractTransactionParams & SigningParams,
): SignedL2Transaction {
  return signTransaction(buildCallContractTransaction(params), params.keyPair);
}

export function sampleCounterCodeHash(): Hash32 {
  return hashDomain("l2.sample.counter.code.v1", []);
}

export function sampleCounterDataHash(counter: UIntLike): Hash32 {
  return hashDomain("l2.sample.counter.data.v1", [uint64Bytes(counter, "counter")]);
}

export function sampleCounterStorageRoot(counter: UIntLike): Hash32 {
  const out = Buffer.alloc(32);
  SAMPLE_COUNTER_STORAGE_PREFIX.copy(out, 0);
  uint64Bytes(counter, "counter").copy(out, 8);
  return out.toString("hex");
}

export function sampleCounterInitialState(counter: UIntLike = 0): SampleCounterState {
  return {
    code_hash: sampleCounterCodeHash(),
    data_hash: sampleCounterDataHash(counter),
    storage_root: sampleCounterStorageRoot(counter),
  };
}

export function readSampleCounterFromAccount(account: L2Account): number {
  if (normalizeHash32(account.code_hash) !== sampleCounterCodeHash()) {
    throw new Error("account is not a sample counter contract");
  }
  const storage = Buffer.from(normalizeHash32(account.storage_root), "hex");
  if (
    !storage.subarray(0, SAMPLE_COUNTER_STORAGE_PREFIX.length).equals(SAMPLE_COUNTER_STORAGE_PREFIX)
  ) {
    throw new Error("sample counter storage root is malformed");
  }
  if (storage.subarray(16).some((byte) => byte !== 0)) {
    throw new Error("sample counter storage root is malformed");
  }
  const counter = storage.readBigUInt64BE(8);
  if (normalizeHash32(account.data_hash) !== sampleCounterDataHash(counter)) {
    throw new Error("sample counter data hash mismatch");
  }
  return toSafeNumber(counter, "counter");
}

export function sampleCounterIncrementBody(increment: UIntLike): Cell {
  return beginCell()
    .storeUint(SAMPLE_COUNTER_INCREMENT_OPCODE, 32)
    .storeUint(toPositiveUint(increment, "increment", 32), 32)
    .endCell();
}

export function sampleCounterIncrementBodyBase64(increment: UIntLike): string {
  return sampleCounterIncrementBody(increment).toBoc().toString("base64");
}

function signTransaction(
  tx: Omit<SignedL2Transaction, "public_key" | "signature">,
  keyPair: nacl.SignKeyPair,
): SignedL2Transaction {
  const unsigned: SignedL2Transaction = {
    ...tx,
    public_key: Buffer.from(keyPair.publicKey).toString("hex"),
    signature: null,
  };
  const signature = nacl.sign.detached(signingPayload(unsigned), keyPair.secretKey);
  return {
    ...unsigned,
    signature: Buffer.from(signature).toString("hex"),
  };
}

function normalizeHash32(value: string): Hash32 {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return cleaned.toLowerCase();
}

function toUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = parseUint(value, field);
  if (parsed >= (1n << BigInt(bits))) {
    throw new Error(`${field} exceeds uint${bits}`);
  }
  return parsed;
}

function toPositiveUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = toUint(value, field, bits);
  if (parsed === 0n) {
    throw new Error(`${field} must be non-zero`);
  }
  return parsed;
}

function parseUint(value: UIntLike, field: string): bigint {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new Error(`${field} must be non-negative`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${field} must be a non-negative safe integer`);
    }
    return BigInt(value);
  }
  if (!/^[0-9]+$/.test(value)) {
    throw new Error(`${field} must be an unsigned decimal integer`);
  }
  return BigInt(value);
}

function toSafeNumber(value: bigint, field: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${field} exceeds JavaScript safe integer range`);
  }
  return Number(value);
}

function toDecimalString(value: bigint): string {
  return value.toString(10);
}

function uint64Bytes(value: UIntLike, field: string): Buffer {
  const parsed = toUint(value, field, 64);
  const out = Buffer.alloc(8);
  out.writeBigUInt64BE(parsed);
  return out;
}

function zeroHash32(): Hash32 {
  return "0".repeat(64);
}

function normalizeSingleRootBocBase64(value: string, field: string): void {
  try {
    const cells = Cell.fromBoc(Buffer.from(value, "base64"));
    if (cells.length !== 1) {
      throw new Error("expected single-root BoC");
    }
  } catch (error) {
    throw new Error(`${field} must be a single-root TON BoC base64 string`, { cause: error });
  }
}
