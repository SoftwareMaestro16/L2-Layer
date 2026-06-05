import {
  ENTROPIS_GAS_LIMIT,
  ENTROPIS_MAX_GAS_PRICE,
  ENTROPIS_TX_TTL_BLOCKS,
  l2RawAddress,
  parseL2Address,
  signTransferTransaction,
  txHash
} from "@/lib/enwallet";
import {
  fetchNetworkSnapshot,
  getAccountNonce,
  pendingTransfer,
  requestProduceBlock
} from "@/lib/l2-api";
import type { TransferReview } from "@/lib/types";

const DEFAULT_API_BASE = "http://127.0.0.1:8080";
const API_BASE = (process.env.NEXT_PUBLIC_ENTROPIS_API_URL ?? DEFAULT_API_BASE).replace(/\/+$/u, "");

export async function prepareEntTransfer(params: {
  recoveryWords: string;
  accountId: string;
  recipient: string;
  amount: string;
  amountBaseUnits: string;
}): Promise<TransferReview> {
  const [nonce, network] = await Promise.all([
    getAccountNonce(params.accountId),
    fetchNetworkSnapshot()
  ]);
  const recipientAccountId = parseL2Address(params.recipient);
  const validUntilBlock = Math.max(0, network.latestBatch) + ENTROPIS_TX_TTL_BLOCKS;
  const signedTx = signTransferTransaction({
    recoveryWords: params.recoveryWords,
    from: params.accountId,
    nonce,
    to: recipientAccountId,
    amount: params.amountBaseUnits,
    validUntilBlock
  });
  const hash = txHash(signedTx);

  return {
    signedTx,
    txHash: hash,
    recipient: l2RawAddress(recipientAccountId),
    recipientAccountId,
    amount: params.amount,
    amountBaseUnits: params.amountBaseUnits,
    assetId: 0,
    symbol: "ENT",
    nonce,
    validUntilBlock,
    gasLimit: ENTROPIS_GAS_LIMIT,
    maxGasPrice: String(ENTROPIS_MAX_GAS_PRICE),
    feeAssetId: 0,
    pending: pendingTransfer(hash, params.recipient, Number(params.amount))
  };
}

export async function submitPreparedEntTransfer(review: TransferReview): Promise<string> {
  const response = await postJson<{ tx_hash: string }>("/v1/tx", review.signedTx);
  if (response.tx_hash !== review.txHash) {
    throw new Error("Node returned an unexpected transaction hash.");
  }
  await requestProduceBlock().catch(() => undefined);
  return response.tx_hash;
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
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
