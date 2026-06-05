import {
  ENWALLET_V5R1_INTERFACE,
  ENWALLET_V5R1_LABEL,
} from "./enwallet.js";

export interface EnWalletV5GetterResult {
  interface: typeof ENWALLET_V5R1_INTERFACE;
  interface_label: typeof ENWALLET_V5R1_LABEL;
  method: string;
  type: string;
  value: string | boolean | number;
}

export function parseEnWalletV5GetterResult(response: {
  method: string;
  result: unknown;
}): EnWalletV5GetterResult {
  const envelope = asRecord(response.result, "EnWallet getter result");
  if (envelope.interface !== ENWALLET_V5R1_INTERFACE) {
    throw new Error("response is not an EnWallet V5 getter result");
  }
  const result = asRecord(envelope.result, "EnWallet getter payload");
  const type = requireString(result.type, "result.type");
  return {
    interface: ENWALLET_V5R1_INTERFACE,
    interface_label: ENWALLET_V5R1_LABEL,
    method: response.method,
    type,
    value: resultValue(result, response.method),
  };
}

function asRecord(value: unknown, field: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${field} must be an object`);
  }
  return value as Record<string, unknown>;
}

function requireString(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${field} must be a non-empty string`);
  }
  return value;
}

function resultValue(result: Record<string, unknown>, method: string): string | boolean | number {
  if (method === "get_extensions") {
    const count = result.count;
    if (!Number.isSafeInteger(count) || Number(count) < 0) {
      throw new Error("result.count must be a non-negative integer");
    }
    return Number(count);
  }
  if (!("value" in result)) {
    throw new Error("result.value is missing");
  }
  const value = result.value;
  if (typeof value !== "string" && typeof value !== "boolean") {
    throw new Error("result.value has unsupported type");
  }
  return value;
}
