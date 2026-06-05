import { Address, beginCell } from "@ton/core";
import { isL2ZeroAddress, parseL2Address, type Hash32 } from "./address.js";

type UIntLike = bigint | number | string;

export interface DepositTonMessageParams {
  vaultAddress: string;
  queryId: UIntLike;
  amount: UIntLike;
  l2Recipient: Hash32;
}

export interface TonConnectMessage {
  address: string;
  amount: string;
  payload: string;
}

export function tonDepositForwardPayload(l2Recipient: Hash32) {
  const recipientAddress = requireNonZeroL2Address(l2Recipient, "l2Recipient");
  const recipient = BigInt(`0x${recipientAddress}`);
  return beginCell().storeUint(recipient, 256).endCell();
}

export function jettonDepositForwardPayload(l2Recipient: Hash32) {
  return tonDepositForwardPayload(l2Recipient);
}

export function encodeDepositTonBody(queryId: UIntLike, amount: UIntLike, l2Recipient: Hash32) {
  const recipientAddress = requireNonZeroL2Address(l2Recipient, "l2Recipient");
  return beginCell()
    .storeUint(0x4c324405, 32)
    .storeUint(toUint(queryId, "queryId", 64), 64)
    .storeCoins(toPositiveUint(amount, "amount", 120))
    .storeUint(BigInt(`0x${recipientAddress}`), 256)
    .endCell();
}

export function depositTonTonConnectMessage(params: DepositTonMessageParams): TonConnectMessage {
  parseTonAddress(params.vaultAddress);
  const amount = toPositiveUint(params.amount, "amount", 120);
  const body = encodeDepositTonBody(params.queryId, amount, params.l2Recipient);
  return {
    address: params.vaultAddress,
    amount: amount.toString(10),
    payload: body.toBoc().toString("base64"),
  };
}

function parseTonAddress(value: string): Address {
  try {
    return Address.parse(value);
  } catch (error) {
    throw new Error(`invalid TON address: ${value}`, { cause: error });
  }
}

function requireNonZeroL2Address(value: string, field: string): Hash32 {
  const parsed = parseL2Address(value);
  if (isL2ZeroAddress(parsed)) {
    throw new Error(`${field} cannot be the reserved zero address`);
  }
  return parsed;
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
