import { z } from "zod";
import {
  accountSchema,
  accountTransactionsSchema,
  contractStateSchema,
  depositStatusSchema,
  explorerSummarySchema,
  pagedBlocksSchema,
  pagedDepositsSchema,
  rawJsonSchema,
  readyzSchema,
  healthSchema,
  transactionDetailSchema,
  withdrawalStatusSchema,
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
      message: safeErrorMessage(body),
    } satisfies ApiErrorPayload;
  }
  return schema.parse(body);
}

export function getHealth(apiBase: string) {
  return fetchJson(apiBase, "/healthz", healthSchema);
}

export function getReadiness(apiBase: string) {
  return fetchJson(apiBase, "/readyz", readyzSchema);
}

export function getExplorerSummary(apiBase: string) {
  return fetchJson(apiBase, "/v1/explorer/summary", explorerSummarySchema);
}

export function getBlocks(apiBase: string, beforeHeight?: number | null, limit = 10) {
  const params = new URLSearchParams({ limit: String(limit) });
  if (beforeHeight !== undefined && beforeHeight !== null) {
    params.set("before_height", String(beforeHeight));
  }
  return fetchJson(apiBase, `/v1/explorer/blocks?${params}`, pagedBlocksSchema);
}

export function getDeposits(apiBase: string, beforeHeight?: number | null, limit = 10) {
  const params = new URLSearchParams({ limit: String(limit) });
  if (beforeHeight !== undefined && beforeHeight !== null) {
    params.set("before_height", String(beforeHeight));
  }
  return fetchJson(apiBase, `/v1/explorer/deposits?${params}`, pagedDepositsSchema);
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

export function getDeposit(apiBase: string, id: string) {
  return fetchJson(
    apiBase,
    `/v1/explorer/deposit/${encodeURIComponent(id)}`,
    depositStatusSchema,
  );
}

export function getWithdrawal(apiBase: string, id: string) {
  return fetchJson(
    apiBase,
    `/v1/explorer/withdrawal/${encodeURIComponent(id)}`,
    withdrawalStatusSchema,
  );
}

export function getContractState(apiBase: string, id: string) {
  return fetchJson(
    apiBase,
    `/v1/contract/${encodeURIComponent(id)}/state`,
    contractStateSchema,
  );
}

export function getBlock(apiBase: string, height: string | number) {
  return fetchJson(apiBase, `/v1/block/${height}`, rawJsonSchema);
}

export function getBlockFinality(apiBase: string, height: string | number) {
  return fetchJson(apiBase, `/v1/block/${height}/finality`, rawJsonSchema);
}

export function getDaPayload(apiBase: string, height: string | number) {
  return fetchJson(apiBase, `/v1/da/batch/${height}`, rawJsonSchema);
}

export async function fetchAppJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { accept: "application/json", ...init?.headers },
  });
  const text = await response.text();
  const body = text ? safeJson(text) : null;
  if (!response.ok) {
    throw {
      status: response.status,
      message: safeErrorMessage(body),
    } satisfies ApiErrorPayload;
  }
  return body as T;
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function safeErrorMessage(body: unknown): string {
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
  return "request failed";
}
