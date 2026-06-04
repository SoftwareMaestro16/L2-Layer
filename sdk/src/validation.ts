import { Address } from "@ton/core";
import type { Hash32, UIntLike } from "./types.js";

export function normalizeHash32(value: string): Hash32 {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return cleaned.toLowerCase();
}

export function parseTonAddress(value: string): Address {
  return Address.parse(value);
}

export function hashToUint256(value: string, field: string): bigint {
  try {
    return BigInt(`0x${normalizeHash32(value)}`);
  } catch {
    throw new Error(`${field} must be a 32-byte hex string`);
  }
}

export function toUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = parseUint(value, field);
  if (parsed >= (1n << BigInt(bits))) {
    throw new Error(`${field} exceeds uint${bits}`);
  }
  return parsed;
}

export function toPositiveUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = toUint(value, field, bits);
  if (parsed === 0n) {
    throw new Error(`${field} must be non-zero`);
  }
  return parsed;
}

export function parseUint(value: UIntLike, field: string): bigint {
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

export function toSafeNumber(value: bigint, field: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${field} exceeds JavaScript safe integer range`);
  }
  return Number(value);
}

export function toDecimalString(value: bigint): string {
  return value.toString(10);
}

export function hexToBytes(value: string, field: string): Buffer {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]+$/.test(cleaned) || cleaned.length % 2 !== 0) {
    throw new Error(`${field} must be hex`);
  }
  return Buffer.from(cleaned, "hex");
}
