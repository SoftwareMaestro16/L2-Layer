import { ENTROPIS_DECIMALS, formatBaseUnits, shortAddress } from "@/lib/enwallet";
import type {
  AssetBalance,
  Collectible,
  TokenHolding,
  WalletTransaction
} from "@/lib/types";

const EMPTY_HASH = "0000000000000000000000000000000000000000000000000000000000000000";

export type ExplorerAccount = {
  account_id: string;
  raw_address: string;
  user_friendly_address: string;
  status: string;
  nonce: number;
  balances: Array<{ asset_id: number; amount: string }>;
};

export type ExplorerTx = {
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

export type ExplorerTxList = {
  items: ExplorerTx[];
};

export type ExplorerSummary = {
  latest_block: { height: number } | null;
  latest_batch_commit: { batch_no: number; status: string } | null;
  latest_finalized_batch: { batch_no: number; status: string } | null;
};

export type BasicAccount = {
  nonce: number;
  balances: Record<string, string | number>;
};

export function entBalance(balances: ExplorerAccount["balances"]): AssetBalance {
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

export function tokenHoldings(balances: ExplorerAccount["balances"]): TokenHolding[] {
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

export function liveCollectibles(): Collectible[] {
  return [];
}

export function mapTransaction(tx: ExplorerTx): WalletTransaction {
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
