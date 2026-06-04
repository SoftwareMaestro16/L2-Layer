import { Address, beginCell, Cell } from "@ton/core";
import { parseL2Address } from "./address.js";

export const JETTON_TRANSFER_OPCODE = 0x0f8a7ea5;

type UIntLike = bigint | number | string;

export interface DepositJettonMessageParams {
  jettonWalletAddress: string;
  vaultAddress: string;
  responseAddress: string;
  queryId: UIntLike;
  jettonAmount: UIntLike;
  forwardTonAmount: UIntLike;
  tonAmount: UIntLike;
  l2Recipient: string;
}

export interface TonConnectMessage {
  address: string;
  amount: string;
  payload: string;
}

export function encodeJettonDepositTransferBody(params: DepositJettonMessageParams): Cell {
  const destination = parseTonAddress(params.vaultAddress);
  const responseDestination = parseTonAddress(params.responseAddress);
  const payload = l2RecipientPayload(params.l2Recipient);

  return beginCell()
    .storeUint(JETTON_TRANSFER_OPCODE, 32)
    .storeUint(toUint(params.queryId, "queryId", 64), 64)
    .storeCoins(toPositiveUint(params.jettonAmount, "jettonAmount", 120))
    .storeAddress(destination)
    .storeAddress(responseDestination)
    .storeBit(false)
    .storeCoins(toPositiveUint(params.forwardTonAmount, "forwardTonAmount", 120))
    .storeBit(true)
    .storeRef(payload)
    .endCell();
}

export function depositJettonTonConnectMessage(
  params: DepositJettonMessageParams,
): TonConnectMessage {
  parseTonAddress(params.jettonWalletAddress);
  const tonAmount = toPositiveUint(params.tonAmount, "tonAmount", 120);
  return {
    address: params.jettonWalletAddress,
    amount: tonAmount.toString(10),
    payload: encodeJettonDepositTransferBody(params).toBoc().toString("base64"),
  };
}

function l2RecipientPayload(l2Recipient: string): Cell {
  return beginCell().storeUint(BigInt(`0x${parseL2Address(l2Recipient)}`), 256).endCell();
}

function parseTonAddress(value: string): Address {
  return Address.parse(value);
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
