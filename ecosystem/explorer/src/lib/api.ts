import { z } from "zod";
import {
  accountSchema,
  accountTransactionsSchema,
  healthSchema,
  transactionDetailSchema,
} from "@/lib/schemas";

export type ApiErrorPayload = {
  message: string;
  status?: number;
};

export function normalizeApiBase(apiBase: string): string {
  return apiBase.replace(/\/+$/, "");
}

export function accountTransactionsPath(
  address: string,
  cursor?: { before_height: number; before_index: number } | null,
  limit = 25,
): string {
  const params = new URLSearchParams({ limit: String(limit) });
  if (cursor) {
    params.set("before_height", String(cursor.before_height));
    params.set("before_index", String(cursor.before_index));
  }
  return `/v1/explorer/account/${encodeURIComponent(address)}/transactions?${params}`;
}

export async function fetchJson<T>(
  apiBase: string,
  path: string,
  schema: z.ZodType<T>,
): Promise<T> {
  const response = await fetch(`${normalizeApiBase(apiBase)}${path}`, {
    headers: { accept: "application/json" },
    signal: AbortSignal.timeout(8_000),
  });
  const text = await response.text();
  const body = text ? safeJson(text) : null;

  if (!response.ok) {
    throw {
      status: response.status,
      message: safeErrorMessage(body, text),
    } satisfies ApiErrorPayload;
  }
  return schema.parse(body);
}

export function getHealth(apiBase: string) {
  return fetchJson(apiBase, "/healthz", healthSchema);
}

export function getAccount(apiBase: string, address: string) {
  return fetchJson(
    apiBase,
    `/v1/explorer/account/${encodeURIComponent(address)}`,
    accountSchema,
  );
}

export function getAccountTransactions(
  apiBase: string,
  address: string,
  cursor?: { before_height: number; before_index: number } | null,
) {
  return fetchJson(
    apiBase,
    accountTransactionsPath(address, cursor),
    accountTransactionsSchema,
  );
}

export function getTransaction(apiBase: string, hash: string) {
  return fetchJson(
    apiBase,
    `/v1/explorer/tx/${encodeURIComponent(hash)}`,
    transactionDetailSchema,
  );
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function safeErrorMessage(body: unknown, fallback: string): string {
  if (
    body &&
    typeof body === "object" &&
    "error" in body &&
    typeof body.error === "string"
  ) {
    return body.error;
  }
  if (
    body &&
    typeof body === "object" &&
    "message" in body &&
    typeof body.message === "string"
  ) {
    return body.message;
  }
  return fallback || "request failed";
}
