import { beginCell, Cell } from "@ton/core";
import nacl from "tweetnacl";
import { isL2ZeroAddress, normalizeHash32, parseL2Address } from "./address.js";
import { signingPayload } from "./consensus.js";

export type Hash32 = string;
type UIntLike = bigint | number | string;

export const SAMPLE_COUNTER_INCREMENT_OPCODE = 0x534c3201;
export const SAMPLE_COUNTER_INCREMENT_GAS = 25;
const SAMPLE_COUNTER_CODE_MAGIC = 0x4c324343;
const SAMPLE_COUNTER_DATA_MAGIC = 0x4c324344;

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
          code_boc_base64: string;
          data_boc_base64: string;
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
  code_boc_base64?: string;
  data_boc_base64?: string;
  last_lt: number;
}

export interface DeployContractTransactionParams {
  chainId: string;
  from: Hash32;
  nonce: UIntLike;
  contract: Hash32;
  codeBocBase64: string;
  dataBocBase64: string;
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
  code_boc_base64: string;
  data_boc_base64: string;
}

export interface SampleCounterReadResponse extends SampleCounterState {
  contract: Hash32;
  contract_raw_address: string;
  contract_friendly_address: string;
  counter: number;
}

export function buildDeployContractTransaction(
  params: DeployContractTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  const contract = parseL2Address(params.contract);
  const from = requireNonZeroL2Address(params.from, "from");
  const codeBocBase64 = normalizeSingleRootBocBase64(params.codeBocBase64, "codeBocBase64");
  const dataBocBase64 = normalizeSingleRootBocBase64(params.dataBocBase64, "dataBocBase64");
  if (isL2ZeroAddress(contract)) {
    throw new Error("contract cannot be the reserved zero address");
  }
  if (contractCellHash(codeBocBase64) === zeroHash32()) {
    throw new Error("codeBocBase64 hash must be non-zero");
  }
  if (contractCellHash(dataBocBase64) === zeroHash32()) {
    throw new Error("dataBocBase64 hash must be non-zero");
  }
  return {
    chain_id: params.chainId,
    from,
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      DeployContract: {
        contract,
        code_boc_base64: codeBocBase64,
        data_boc_base64: dataBocBase64,
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
  const contract = requireNonZeroL2Address(params.contract, "contract");
  const from = requireNonZeroL2Address(params.from, "from");
  normalizeSingleRootBocBase64(params.bodyBocBase64, "bodyBocBase64");
  return {
    chain_id: params.chainId,
    from,
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      CallContract: {
        contract,
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
  return contractCellHash(sampleCounterCodeBocBase64());
}

export function sampleCounterCodeBocBase64(): string {
  return beginCell().storeUint(SAMPLE_COUNTER_CODE_MAGIC, 32).endCell().toBoc().toString("base64");
}

export function sampleCounterDataHash(counter: UIntLike): Hash32 {
  return contractCellHash(sampleCounterDataBocBase64(counter));
}

export function sampleCounterStorageRoot(counter: UIntLike): Hash32 {
  return sampleCounterDataHash(counter);
}

export function sampleCounterDataBocBase64(counter: UIntLike): string {
  return beginCell()
    .storeUint(SAMPLE_COUNTER_DATA_MAGIC, 32)
    .storeUint(toUint(counter, "counter", 64), 64)
    .endCell()
    .toBoc()
    .toString("base64");
}

export function sampleCounterInitialState(counter: UIntLike = 0): SampleCounterState {
  const code_boc_base64 = sampleCounterCodeBocBase64();
  const data_boc_base64 = sampleCounterDataBocBase64(counter);
  const data_hash = contractCellHash(data_boc_base64);
  return {
    code_hash: contractCellHash(code_boc_base64),
    data_hash,
    storage_root: data_hash,
    code_boc_base64,
    data_boc_base64,
  };
}

export function readSampleCounterFromAccount(account: L2Account): number {
  if (normalizeHash32(account.code_hash) !== sampleCounterCodeHash()) {
    throw new Error("account is not a sample counter contract");
  }
  if (!account.data_boc_base64) {
    throw new Error("account has no sample counter data BoC");
  }
  const counter = decodeSampleCounterDataBoc(account.data_boc_base64);
  if (normalizeHash32(account.data_hash) !== sampleCounterDataHash(counter)) {
    throw new Error("sample counter data hash mismatch");
  }
  if (normalizeHash32(account.storage_root) !== sampleCounterStorageRoot(counter)) {
    throw new Error("sample counter storage root mismatch");
  }
  return toSafeNumber(counter, "counter");
}

export function contractCellHash(bocBase64: string): Hash32 {
  const cell = singleRootCell(bocBase64, "bocBase64");
  return cell.hash().toString("hex");
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

function zeroHash32(): Hash32 {
  return "0".repeat(64);
}

function requireNonZeroL2Address(value: string, field: string): Hash32 {
  const parsed = parseL2Address(value);
  if (isL2ZeroAddress(parsed)) {
    throw new Error(`${field} cannot be the reserved zero address`);
  }
  return parsed;
}

function normalizeSingleRootBocBase64(value: string, field: string): string {
  const cell = singleRootCell(value, field);
  return cell.toBoc().toString("base64");
}

function singleRootCell(value: string, field: string): Cell {
  try {
    const cells = Cell.fromBoc(Buffer.from(value, "base64"));
    if (cells.length !== 1) {
      throw new Error("expected single-root BoC");
    }
    return cells[0];
  } catch (error) {
    throw new Error(`${field} must be a single-root TON BoC base64 string`, { cause: error });
  }
}

function decodeSampleCounterDataBoc(value: string): bigint {
  const slice = singleRootCell(value, "data_boc_base64").beginParse();
  if (slice.loadUint(32) !== SAMPLE_COUNTER_DATA_MAGIC) {
    throw new Error("sample counter data BoC is malformed");
  }
  const counter = slice.loadUintBig(64);
  if (slice.remainingBits !== 0 || slice.remainingRefs !== 0) {
    throw new Error("sample counter data BoC is malformed");
  }
  return counter;
}
