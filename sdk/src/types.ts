import type nacl from "tweetnacl";

export type Hash32 = string;
export type UIntLike = bigint | number | string;

export type L2TransactionKind =
  | {
      Deposit: {
        deposit_id: Hash32;
        asset_id: number;
        recipient: Hash32;
        amount: string;
      };
    }
  | { Transfer: { to: Hash32; asset_id: number; amount: string } }
  | { Withdraw: { asset_id: number; amount: string; l1_recipient: string } }
  | { CallContract: { contract: Hash32; body_boc_base64: string } };

export interface SignedL2Transaction {
  chain_id: string;
  from: Hash32 | null;
  nonce: number;
  gas_limit: number;
  max_gas_price: string;
  kind: L2TransactionKind;
  public_key: string | null;
  signature: string | null;
}

export interface DepositEvent {
  deposit_id: Hash32;
  asset_id: number;
  recipient: Hash32;
  amount: string;
  l1_tx_hash: Hash32;
  l1_lt: number;
}

export interface SubmitTxResponse {
  tx_hash: Hash32;
}

export interface L2Account {
  nonce: number;
  balances: Record<string, string | number>;
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
  last_lt: number;
}

export interface EntFaucetResponse {
  account_id: Hash32;
  amount_ent: string;
  amount_base_units: string;
  deposit_id: Hash32;
  granted: boolean;
}

export interface TransferTransactionParams {
  chainId: string;
  from: Hash32;
  nonce: UIntLike;
  to: Hash32;
  assetId: UIntLike;
  amount: UIntLike;
  gasLimit: UIntLike;
  maxGasPrice: UIntLike;
}

export interface WithdrawTransactionParams {
  chainId: string;
  from: Hash32;
  nonce: UIntLike;
  assetId: UIntLike;
  amount: UIntLike;
  l1Recipient: string;
  gasLimit: UIntLike;
  maxGasPrice: UIntLike;
}

export interface SigningParams {
  keyPair: nacl.SignKeyPair;
}

export interface WithdrawalProofLeaf {
  withdrawal_id: Hash32;
  asset_id: number;
  amount: string;
  l2_sender: Hash32;
  l1_recipient: string;
}

export interface WithdrawalMerkleProof {
  leaf_index: number;
  siblings: Hash32[];
}

export interface WithdrawalProofResponse {
  block_height: number;
  withdrawal_root: Hash32;
  leaf: WithdrawalProofLeaf;
  proof: WithdrawalMerkleProof;
}

export interface TonConnectMessage {
  address: string;
  amount: string;
  payload: string;
}

export interface ClaimWithdrawalTonConnectMessageParams {
  rollupRootAddress: string;
  proof: WithdrawalProofResponse;
  amount: UIntLike;
}

export interface DepositTonMessageParams {
  vaultAddress: string;
  queryId: UIntLike;
  amount: UIntLike;
  l2Recipient: Hash32;
}

export interface TonL2ClientOptions {
  adminToken?: string;
}
