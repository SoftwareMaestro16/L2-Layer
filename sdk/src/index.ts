export * as AssetVaultL1 from "./generated/AssetVault.gen.js";
export * as RollupRootL1 from "./generated/RollupRoot.gen.js";
export {
  accountLeafHash,
  blockHeaderHash,
  canonicalBatchDataHash,
  CONSENSUS_ENCODING_VERSION,
  deriveAccountId,
  encodeAccountLeaf,
  encodeBatchData,
  encodeBlockHeader,
  encodeReceipt,
  encodeSignedTransaction,
  encodeUnsignedTransaction,
  encodeWithdrawalLeaf,
  hashDomain,
  receiptLeafHash,
  sha256Hex,
  signingPayload,
  txHash,
  withdrawalId,
  withdrawalLeafHash,
} from "./consensus.js";
export type { AccountLeaf, L2BlockHeader, Receipt, WithdrawalLeaf } from "./consensus.js";
export {
  accountIdFromKeyPair,
  accountIdFromPublicKey,
  buildTransferTransaction,
  buildWithdrawTransaction,
  signTransaction,
  signTransferTransaction,
  signWithdrawTransaction,
} from "./transactions.js";
export {
  buildClaimWithdrawalBody,
  claimWithdrawalTonConnectMessage,
  depositTonTonConnectMessage,
  encodeDepositTonBody,
  jettonDepositForwardPayload,
  releaseAuthorizedCell,
  tonDepositForwardPayload,
  withdrawalMerkleProofCell,
} from "./bridge.js";
export { EntropisApiError, EntropisClient, TonL2Client } from "./client.js";
export { normalizeHash32, parseTonAddress } from "./validation.js";
export type {
  ClaimWithdrawalTonConnectMessageParams,
  DepositEvent,
  DepositTonMessageParams,
  EntFaucetResponse,
  Hash32,
  L2Account,
  L2TransactionKind,
  SignedL2Transaction,
  SubmitTxResponse,
  TonConnectMessage,
  TonL2ClientOptions,
  TransferTransactionParams,
  UIntLike,
  WithdrawalMerkleProof,
  WithdrawalProofLeaf,
  WithdrawalProofResponse,
  WithdrawTransactionParams,
} from "./types.js";

export const L2_NATIVE_GAS_ASSET = 0;
