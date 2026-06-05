import {
  l2RawAddress,
  parseL2Address,
  shortAddress,
  signTransferTransaction,
  type SignedL2Transaction
} from "@/lib/enwallet";
import {
  entBalance,
  liveCollectibles,
  mapTransaction,
  tokenHoldings,
  type BasicAccount,
  type ExplorerAccount,
  type ExplorerSummary,
  type ExplorerTxList
} from "@/lib/l2-api-model";
import type { NetworkSnapshot, WalletLiveData, WalletTransaction } from "@/lib/types";

const DEFAULT_API_BASE = "http://127.0.0.1:8080";
const API_BASE = (process.env.NEXT_PUBLIC_ENTROPIS_API_URL ?? DEFAULT_API_BASE).replace(/\/+$/u, "");

export async function fetchWalletLiveData(accountId: string): Promise<WalletLiveData> {
  const [account, transactions] = await Promise.all([
    fetchExplorerAccount(accountId),
    fetchAccountTransactions(accountId)
  ]);
  return {
    balance: entBalance(account?.balances ?? []),
    tokens: tokenHoldings(account?.balances ?? []),
    collectibles: liveCollectibles(),
    transactions
  };
}

export async function fetchNetworkSnapshot(): Promise<NetworkSnapshot> {
  try {
    const [ready, summary] = await Promise.all([fetchJson<{ status: string }>("/readyz"), fetchExplorerSummary()]);
    return {
      chainId: "entropis-testnet",
      latestBatch: summary.latest_batch_commit?.batch_no ?? summary.latest_block?.height ?? 0,
      finality: summary.latest_finalized_batch
        ? `Finalized batch ${summary.latest_finalized_batch.batch_no}`
        : "Waiting for finalized batch",
      status: ready.status === "ready" ? "ready" : "degraded"
    };
  } catch {
    return {
      chainId: "entropis-testnet",
      latestBatch: 0,
      finality: "Node is offline",
      status: "offline"
    };
  }
}

export async function getAccountNonce(accountId: string): Promise<number> {
  try {
    const account = await fetchJson<BasicAccount>(`/v1/account/${encodeURIComponent(l2RawAddress(accountId))}`);
    return account.nonce;
  } catch (error) {
    if (isNotFound(error)) {
      return 0;
    }
    throw error;
  }
}

export async function submitEntTransfer(params: {
  recoveryWords: string;
  accountId: string;
  nonce: number;
  recipient: string;
  amountBaseUnits: string;
}): Promise<string> {
  const tx = signTransferTransaction({
    recoveryWords: params.recoveryWords,
    from: params.accountId,
    nonce: params.nonce,
    to: params.recipient,
    amount: params.amountBaseUnits
  });
  const response = await postJson<{ tx_hash: string }>("/v1/tx", tx);
  await requestProduceBlock().catch(() => undefined);
  return response.tx_hash;
}

export async function requestEntFaucet(accountId: string): Promise<void> {
  await fetchAppJson("/api/faucet", {
    method: "POST",
    body: JSON.stringify({ account_id: l2RawAddress(parseL2Address(accountId)) })
  });
}

export async function requestProduceBlock(): Promise<void> {
  await fetchAppJson("/api/produce-block", { method: "POST" });
}

export function pendingTransfer(txHash: string, recipient: string, amount: number): WalletTransaction {
  return {
    id: txHash,
    type: "send",
    status: "pending",
    title: "Submitted ENT transfer",
    counterparty: shortAddress(recipient),
    amount: -amount,
    symbol: "ENT",
    fee: 0,
    timestamp: new Date().toISOString(),
    memo: "Waiting for L2 block inclusion"
  };
}

async function fetchExplorerAccount(accountId: string): Promise<ExplorerAccount | null> {
  try {
    return await fetchJson<ExplorerAccount>(
      `/v1/explorer/account/${encodeURIComponent(l2RawAddress(parseL2Address(accountId)))}`
    );
  } catch (error) {
    if (isNotFound(error)) {
      return null;
    }
    throw error;
  }
}

async function fetchAccountTransactions(accountId: string): Promise<WalletTransaction[]> {
  try {
    const response = await fetchJson<ExplorerTxList>(
      `/v1/explorer/account/${encodeURIComponent(l2RawAddress(parseL2Address(accountId)))}/transactions?limit=50`
    );
    return response.items.map(mapTransaction);
  } catch (error) {
    if (isNotFound(error)) {
      return [];
    }
    throw error;
  }
}

async function fetchExplorerSummary(): Promise<ExplorerSummary> {
  return fetchJson<ExplorerSummary>("/v1/explorer/summary");
}

async function fetchJson<T>(path: string): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`);
  if (!response.ok) {
    throw new ApiError(response.status, await response.text());
  }
  return response.json() as Promise<T>;
}

async function postJson<T>(path: string, body: SignedL2Transaction): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  if (!response.ok) {
    throw new ApiError(response.status, await response.text());
  }
  return response.json() as Promise<T>;
}

async function fetchAppJson(path: string, init: RequestInit): Promise<unknown> {
  const response = await fetch(path, {
    ...init,
    headers: { "content-type": "application/json", ...init.headers }
  });
  if (!response.ok) {
    throw new ApiError(response.status, await response.text());
  }
  const text = await response.text();
  return text ? JSON.parse(text) : undefined;
}

function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
}

class ApiError extends Error {
  constructor(
    public readonly status: number,
    body: string
  ) {
    super(publicMessage(body) || `Request failed with status ${status}`);
  }
}

function publicMessage(body: string): string {
  try {
    const parsed = JSON.parse(body) as { error?: unknown };
    return typeof parsed.error === "string" ? parsed.error : body;
  } catch {
    return body;
  }
}
