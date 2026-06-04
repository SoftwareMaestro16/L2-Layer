import {
  ENTROPIS_DECIMALS,
  formatBaseUnits,
  l2RawAddress,
  parseL2Address,
  shortAddress,
  signTransferTransaction,
  type SignedL2Transaction
} from "@/lib/enwallet";
import type {
  AssetBalance,
  Collectible,
  NetworkSnapshot,
  TokenHolding,
  WalletLiveData,
  WalletTransaction
} from "@/lib/types";

const DEFAULT_API_BASE = "http://127.0.0.1:8080";
const API_BASE = (process.env.NEXT_PUBLIC_ENTROPIS_API_URL ?? DEFAULT_API_BASE).replace(/\/+$/u, "");
const EMPTY_HASH = "0000000000000000000000000000000000000000000000000000000000000000";

type ExplorerAccount = {
  account_id: string;
  raw_address: string;
  user_friendly_address: string;
  status: string;
  nonce: number;
  balances: Array<{ asset_id: number; amount: string }>;
};

type ExplorerTx = {
  block_height: number;
  tx_index: number;
  timestamp: number;
  tx_hash: string;
  kind: string;
  direction: string;
  participants: Array<{
    role: string;
    account_id: string;
    raw_address: string;
    user_friendly_address: string;
  }>;
  asset_id: number | null;
  amount: string | null;
  status: string;
  gas_charged: string | null;
  reason: string | null;
  withdrawal_id: string | null;
};

type ExplorerTxList = {
  items: ExplorerTx[];
};

type ExplorerSummary = {
  latest_block: { height: number } | null;
  latest_batch_commit: { batch_no: number; status: string } | null;
  latest_finalized_batch: { batch_no: number; status: string } | null;
};

type BasicAccount = {
  nonce: number;
  balances: Record<string, string | number>;
};

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

function entBalance(balances: ExplorerAccount["balances"]): AssetBalance {
  const ent = balances.find((balance) => balance.asset_id === 0);
  return {
    assetId: 0,
    symbol: "ENT",
    name: "Entropis",
    amount: formatBaseUnits(ent?.amount ?? "0"),
    baseUnits: ent?.amount ?? "0",
    decimals: ENTROPIS_DECIMALS
  };
}

function tokenHoldings(balances: ExplorerAccount["balances"]): TokenHolding[] {
  const sorted = [...balances].sort((left, right) => left.asset_id - right.asset_id);
  return sorted.map((balance) => {
    const meta = assetMeta(balance.asset_id);
    return {
      id: `asset-${balance.asset_id}`,
      assetId: balance.asset_id,
      symbol: meta.symbol,
      name: meta.name,
      amount: formatBaseUnits(balance.amount, meta.decimals),
      baseUnits: balance.amount,
      decimals: meta.decimals,
      color: meta.color
    };
  });
}

function liveCollectibles(): Collectible[] {
  return [];
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

function mapTransaction(tx: ExplorerTx): WalletTransaction {
  const meta = assetMeta(tx.asset_id ?? 0);
  const amount = tx.amount ? formatBaseUnits(tx.amount, meta.decimals) : 0;
  const outgoing = tx.direction === "out";
  const self = tx.direction === "self";
  const signedAmount = outgoing ? -amount : self ? 0 : amount;
  const counterparty =
    tx.participants.find((participant) => participant.role !== "from") ??
    tx.participants[0] ?? {
      user_friendly_address: EMPTY_HASH
    };

  return {
    id: tx.tx_hash,
    type: transactionType(tx.kind, tx.direction),
    status: tx.status === "applied" ? "confirmed" : tx.status === "rejected" ? "failed" : "pending",
    title: transactionTitle(tx.kind, tx.direction),
    counterparty: shortAddress(counterparty.user_friendly_address),
    amount: signedAmount,
    symbol: meta.symbol,
    fee: formatBaseUnits(tx.gas_charged ?? "0"),
    timestamp: new Date(tx.timestamp * 1000).toISOString(),
    memo: tx.reason ?? tx.withdrawal_id ?? undefined
  };
}

function transactionType(kind: string, direction: string): WalletTransaction["type"] {
  if (kind === "deposit") {
    return "deposit";
  }
  if (kind === "withdraw") {
    return "withdraw";
  }
  if (kind === "call_contract" || kind === "deploy_contract") {
    return "contract";
  }
  return direction === "in" ? "receive" : "send";
}

function transactionTitle(kind: string, direction: string): string {
  if (kind === "deposit") {
    return "Bridge deposit";
  }
  if (kind === "withdraw") {
    return "Withdrawal";
  }
  if (kind === "call_contract") {
    return "Contract call";
  }
  if (kind === "deploy_contract") {
    return "Contract deploy";
  }
  if (direction === "self") {
    return "Self transfer";
  }
  return direction === "in" ? "Received transfer" : "Sent transfer";
}

function assetMeta(assetId: number) {
  if (assetId === 0) {
    return {
      symbol: "ENT",
      name: "Entropis",
      decimals: ENTROPIS_DECIMALS,
      color: "from-cyan-500 to-violet-600"
    };
  }
  if (assetId === 1) {
    return {
      symbol: "TON",
      name: "TON testnet",
      decimals: 9,
      color: "from-sky-500 to-blue-600"
    };
  }
  return {
    symbol: `A${assetId}`,
    name: `L2 asset ${assetId}`,
    decimals: 9,
    color: "from-emerald-500 to-cyan-600"
  };
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
