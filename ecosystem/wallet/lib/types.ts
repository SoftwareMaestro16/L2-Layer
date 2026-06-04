export type WalletAccount = {
  id: string;
  label: string;
  address: string;
  shortAddress: string;
  network: "Entropis localnet";
  createdFrom: "created" | "imported";
};

export type AssetBalance = {
  assetId: "ENT";
  symbol: "ENT";
  name: "Entropis";
  amount: number;
  fiatValue: number;
  change24h: number;
};

export type MockTransaction = {
  id: string;
  type: "send" | "receive" | "deposit" | "fee";
  status: "confirmed" | "pending" | "failed";
  title: string;
  counterparty: string;
  amount: number;
  fee: number;
  timestamp: string;
  memo?: string;
};

export type WalletSession = {
  account: WalletAccount;
  balance: AssetBalance;
  transactions: MockTransaction[];
};

export type SendTransferInput = {
  recipient: string;
  amount: string;
  memo?: string;
};

export type NetworkSnapshot = {
  chainId: string;
  latestBatch: number;
  finality: string;
  status: "mock-online";
};
