import type { SignedL2Transaction } from "@/lib/enwallet";

export type WalletAccount = {
  accountId: string;
  rawAddress: string;
  label: string;
  address: string;
  shortAddress: string;
  network: "Entropis testnet";
  createdFrom: "created" | "imported";
  publicKey: string;
};

export type AssetBalance = {
  assetId: number;
  symbol: string;
  name: "Entropis";
  amount: number;
  baseUnits: string;
  decimals: number;
};

export type TokenHolding = {
  id: string;
  assetId: number;
  symbol: string;
  name: string;
  amount: number;
  baseUnits: string;
  decimals: number;
  color: string;
};

export type Collectible = {
  id: string;
  name: string;
  collection: string;
  rarity: string;
  accent: string;
};

export type WalletTransaction = {
  id: string;
  type: "send" | "receive" | "deposit" | "withdraw" | "contract" | "fee";
  status: "confirmed" | "pending" | "failed";
  title: string;
  counterparty: string;
  amount: number;
  symbol: string;
  fee: number;
  timestamp: string;
  memo?: string;
};

export type WalletSession = {
  account: WalletAccount;
  recoveryWords: string;
};

export type SendTransferInput = {
  recipient: string;
  amount: string;
  memo?: string;
};

export type TransferReview = {
  signedTx: SignedL2Transaction;
  txHash: string;
  recipient: string;
  recipientAccountId: string;
  amount: string;
  amountBaseUnits: string;
  assetId: number;
  symbol: "ENT";
  nonce: number;
  validUntilBlock: number;
  gasLimit: number;
  maxGasPrice: string;
  feeAssetId: number;
  pending: WalletTransaction;
};

export type NetworkSnapshot = {
  chainId: string;
  latestBatch: number;
  finality: string;
  status: "ready" | "offline" | "degraded";
};

export type WalletLiveData = {
  balance: AssetBalance;
  transactions: WalletTransaction[];
  tokens: TokenHolding[];
  collectibles: Collectible[];
};
