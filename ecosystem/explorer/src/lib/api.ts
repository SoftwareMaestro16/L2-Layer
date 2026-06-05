import type { z } from "zod";
import {
  accountSchema,
  assetsSchema,
  codeSchema,
  explorerSummarySchema,
  sourceSchema,
  transactionDetailSchema,
  transactionListSchema
} from "@/lib/schemas";

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string
  ) {
    super(message);
  }
}

export async function fetchSummary(apiBase: string) {
  return fetchJson(apiBase, "/v1/explorer/summary", explorerSummarySchema);
}

export async function fetchAccount(apiBase: string, address: string) {
  return fetchJson(apiBase, `/v1/explorer/account/${encodeURIComponent(address)}`, accountSchema);
}

export async function fetchAccountAssets(apiBase: string, address: string) {
  return fetchJson(apiBase, `/v1/explorer/account/${encodeURIComponent(address)}/assets`, assetsSchema);
}

export async function fetchAccountCode(apiBase: string, address: string) {
  return fetchJson(apiBase, `/v1/explorer/account/${encodeURIComponent(address)}/code`, codeSchema);
}

export async function fetchTransactions(
  apiBase: string,
  address: string,
  cursor?: { before_height: number; before_index: number } | null
) {
  const params = new URLSearchParams({ limit: "25" });
  if (cursor) {
    params.set("before_height", String(cursor.before_height));
    params.set("before_index", String(cursor.before_index));
  }
  return fetchJson(
    apiBase,
    `/v1/explorer/account/${encodeURIComponent(address)}/transactions?${params}`,
    transactionListSchema
  );
}

export async function fetchTransaction(apiBase: string, hash: string) {
  return fetchJson(apiBase, `/v1/explorer/tx/${encodeURIComponent(hash)}`, transactionDetailSchema);
}

export async function submitVerifier(
  apiBase: string,
  body: { account_id?: string; code_hash?: string; files: Array<{ path: string; content: string }> }
) {
  return postJson(apiBase, "/v1/explorer/verifier/submissions", body, sourceSchema);
}

async function fetchJson<T extends z.ZodType>(
  apiBase: string,
  path: string,
  schema: T
): Promise<z.infer<T>> {
  const response = await fetch(`${apiBase}${path}`);
  const text = await response.text();
  if (!response.ok) {
    throw new ApiError(response.status, publicMessage(text));
  }
  return schema.parse(JSON.parse(text));
}

async function postJson<T extends z.ZodType>(
  apiBase: string,
  path: string,
  body: unknown,
  schema: T
): Promise<z.infer<T>> {
  const response = await fetch(`${apiBase}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  const text = await response.text();
  if (!response.ok) {
    throw new ApiError(response.status, publicMessage(text));
  }
  return schema.parse(JSON.parse(text));
}

function publicMessage(text: string): string {
  try {
    const parsed = JSON.parse(text) as { error?: unknown };
    return typeof parsed.error === "string" ? parsed.error : text;
  } catch {
    return text || "Request failed";
  }
}
