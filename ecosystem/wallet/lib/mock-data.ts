import type {
  AssetBalance,
  Collectible,
  MockTransaction,
  NetworkSnapshot,
  TokenHolding,
  WalletAccount,
  WalletSession
} from "@/lib/types";

export const mockAccountBase: Omit<WalletAccount, "createdFrom"> = {
  id: "acct_demo_entropis_001",
  label: "Entropis Demo",
  address: "EXb71Pq2mNwQhJ9Vf5xLk8Cc2tR0sZm4Yd6Ae3TnU",
  shortAddress: "EXb71Pq2...Ae3TnU",
  network: "Entropis localnet"
};

export const mockBalance: AssetBalance = {
  assetId: "ENT",
  symbol: "ENT",
  name: "Entropis",
  amount: 1284.642,
  fiatValue: 642.32,
  change24h: 2.8
};

export const mockTransactions: MockTransaction[] = [
  {
    id: "tx_mock_1051",
    type: "receive",
    status: "confirmed",
    title: "Received ENT",
    counterparty: "EX91wR7m...Hf3pQa",
    amount: 220,
    fee: 0,
    timestamp: "2026-06-05T00:44:00.000Z",
    memo: "Faucet grant mock"
  },
  {
    id: "tx_mock_1048",
    type: "send",
    status: "confirmed",
    title: "Sent ENT",
    counterparty: "EXd8Jf6s...Qp8zLm",
    amount: -42.5,
    fee: 0.012,
    timestamp: "2026-06-04T21:18:00.000Z",
    memo: "UI prototype payment"
  },
  {
    id: "tx_mock_1040",
    type: "deposit",
    status: "confirmed",
    title: "Mock bridge deposit",
    counterparty: "AssetVault testnet placeholder",
    amount: 1000,
    fee: 0.034,
    timestamp: "2026-06-04T18:03:00.000Z"
  },
  {
    id: "tx_mock_1037",
    type: "fee",
    status: "confirmed",
    title: "Sequencer fee",
    counterparty: "Entropis sequencer",
    amount: -0.146,
    fee: 0,
    timestamp: "2026-06-04T17:46:00.000Z"
  }
];

export const mockTokens: TokenHolding[] = [
  {
    id: "token-ent",
    symbol: "ENT",
    name: "Entropis",
    amount: 1284.642,
    fiatValue: 642.32,
    color: "from-blue-500 to-violet-600"
  },
  {
    id: "token-stent",
    symbol: "stENT",
    name: "Staked ENT",
    amount: 318.2,
    fiatValue: 190.92,
    color: "from-indigo-500 to-fuchsia-500"
  },
  {
    id: "token-demo",
    symbol: "DEMO",
    name: "Demo Jetton",
    amount: 4200,
    fiatValue: 84,
    color: "from-sky-500 to-purple-500"
  }
];

export const mockCollectibles: Collectible[] = [
  {
    id: "nft-validator-pass",
    name: "Validator Pass #018",
    collection: "Entropis Access",
    rarity: "Rare",
    accent: "from-blue-500 via-indigo-500 to-violet-600"
  },
  {
    id: "nft-genesis-node",
    name: "Genesis Node #204",
    collection: "L2 Operators",
    rarity: "Epic",
    accent: "from-violet-500 via-purple-500 to-blue-500"
  }
];

export const mockNetworkSnapshot: NetworkSnapshot = {
  chainId: "entropis-localnet-mock",
  latestBatch: 1842,
  finality: "Mock finality: 2 batches",
  status: "mock-online"
};

export function createMockSession(createdFrom: WalletAccount["createdFrom"]): WalletSession {
  return {
    account: {
      ...mockAccountBase,
      createdFrom
    },
    balance: {
      ...mockBalance
    },
    transactions: mockTransactions.map((transaction) => ({ ...transaction })),
    tokens: mockTokens.map((token) => ({ ...token })),
    collectibles: mockCollectibles.map((collectible) => ({ ...collectible }))
  };
}
