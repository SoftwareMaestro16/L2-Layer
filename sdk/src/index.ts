import { Address, beginCell, Cell } from "@ton/core";
import nacl from "tweetnacl";
import {
  deriveAccountId as deriveAccountIdFromBytes,
  signingPayload,
} from "./consensus.js";
import {
  isL2ZeroAddress,
  l2RawAddress,
  normalizeHash32,
  parseL2Address,
  type Hash32,
} from "./address.js";
import { EntropisApiError } from "./api_error.js";
import type { TonConnectMessage } from "./deposit.js";
import {
  signCallContractTransaction,
  signDeployContractTransaction,
  type CallContractTransactionParams,
  type DeployContractTransactionParams,
  type SampleCounterReadResponse,
} from "./contracts.js";
import * as RollupRootGenerated from "./generated/RollupRoot.gen.js";

export {
  depositJettonTonConnectMessage,
  encodeJettonDepositTransferBody,
  JETTON_TRANSFER_OPCODE,
} from "./jetton.js";
export type { DepositJettonMessageParams } from "./jetton.js";
export {
  depositTonTonConnectMessage,
  encodeDepositTonBody,
  jettonDepositForwardPayload,
  tonDepositForwardPayload,
} from "./deposit.js";
export type { DepositTonMessageParams, TonConnectMessage } from "./deposit.js";
export { EntropisApiError } from "./api_error.js";
export {
  L2_RAW_ADDRESS_PREFIX,
  L2_USER_FRIENDLY_LENGTH,
  L2_USER_FRIENDLY_PREFIX,
  L2_ZERO_ACCOUNT_ID,
  L2_ZERO_ADDRESS_INTERFACE,
  L2_ZERO_ADDRESS_LABEL,
  L2_ZERO_FRIENDLY_ADDRESS,
  L2_ZERO_RAW_ADDRESS,
  isL2ZeroAddress,
  l2RawAddress,
  l2UserFriendlyAddress,
  normalizeHash32,
  parseL2Address,
} from "./address.js";
export type { Hash32 } from "./address.js";
export {
  buildCallContractTransaction,
  buildDeployContractTransaction,
  contractCellHash,
  readSampleCounterFromAccount,
  sampleCounterCodeBocBase64,
  sampleCounterCodeHash,
  sampleCounterDataBocBase64,
  sampleCounterDataHash,
  sampleCounterIncrementBody,
  sampleCounterIncrementBodyBase64,
  sampleCounterInitialState,
  sampleCounterStorageRoot,
  signCallContractTransaction,
  signDeployContractTransaction,
  SAMPLE_COUNTER_INCREMENT_GAS,
  SAMPLE_COUNTER_INCREMENT_OPCODE,
} from "./contracts.js";
export type {
  CallContractTransactionParams,
  DeployContractTransactionParams,
  SampleCounterReadResponse,
  SampleCounterState,
} from "./contracts.js";
export * as AssetVaultL1 from "./generated/AssetVault.gen.js";
export * as RollupRootL1 from "./generated/RollupRoot.gen.js";
export * as EnWalletV5Generated from "./generated/EnWalletV5.gen.js";
export {
  createEnWalletMnemonic,
  enwalletKeyPairFromMnemonic,
  enwalletV5AccountId,
  enwalletV5ContractSalt,
  enwalletV5CodeBocBase64,
  enwalletV5CodeHash,
  enwalletV5DataBocBase64,
  enwalletV5DataCell,
  enwalletV5InitialState,
  enwalletV5SignedExternalBodyBase64,
  enwalletV5SignedInternalBodyBase64,
  enwalletV5StateInitCell,
  signEnWalletV5InitTransaction,
  validateEnWalletMnemonic,
  ENWALLET_V5R1_INTERFACE,
  ENWALLET_V5R1_LABEL,
  ENWALLET_V5R1_TESTNET_WALLET_ID,
} from "./enwallet.js";
export type { EnWalletV5DeployParams, EnWalletV5InitialState, EnWalletV5InitParams } from "./enwallet.js";
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

export const L2_NATIVE_GAS_ASSET = 0;
export const L2_NATIVE_GAS_TOKEN_SYMBOL = "ENT";
const WITHDRAWAL_PROOF_CHUNK_MAX = 3;

type UIntLike = bigint | number | string;

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
  | {
      DeployContract: {
        contract: Hash32;
        code_boc_base64: string;
        data_boc_base64: string;
      };
    }
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
  code_boc_base64?: string;
  data_boc_base64?: string;
  last_lt: number;
}

export interface EntFaucetResponse {
  account_id: Hash32;
  account_raw_address: string;
  account_friendly_address: string;
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

export interface ClaimWithdrawalTonConnectMessageParams {
  rollupRootAddress: string;
  proof: WithdrawalProofResponse;
  amount: UIntLike;
}

export interface TonL2ClientOptions {
  adminToken?: string;
}

export interface ContractGetMethodResponse {
  contract: Hash32;
  contract_raw_address: string;
  contract_friendly_address: string;
  method: string;
  result: unknown;
  source: string;
}

export interface ExplorerInterface {
  id: string;
  label: string;
}

export interface ExplorerBalance {
  asset_id: number;
  amount: string;
}

export interface ExplorerAccountResponse {
  account_id: Hash32;
  raw_address: string;
  user_friendly_address: string;
  status: string;
  nonce: number;
  balances: ExplorerBalance[];
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
  interfaces: ExplorerInterface[];
  last_lt: number;
}

export interface ExplorerTransactionSummary {
  block_height: number;
  tx_index: number;
  timestamp: number;
  block_hash: Hash32;
  tx_hash: Hash32;
  kind: string;
  interface: string | null;
  interface_label: string | null;
  operation: string | null;
  direction: string;
  participants: unknown[];
  asset_id: number | null;
  amount: string | null;
  status: string;
  gas_charged: string | null;
  reason: string | null;
  withdrawal_id: Hash32 | null;
}

export interface ExplorerAccountTransactionsResponse {
  items: ExplorerTransactionSummary[];
  next_cursor: { before_height: number; before_index: number } | null;
}

export function signTransaction(
  tx: Omit<SignedL2Transaction, "public_key" | "signature">,
  keyPair: nacl.SignKeyPair,
): SignedL2Transaction {
  const unsigned: SignedL2Transaction = {
    ...tx,
    public_key: Buffer.from(keyPair.publicKey).toString("hex"),
    signature: null,
  };
  const signature = nacl.sign.detached(signingPayload(unsigned), keyPair.secretKey);
  return {
    ...unsigned,
    signature: Buffer.from(signature).toString("hex"),
  };
}

export function accountIdFromPublicKey(publicKey: Uint8Array | string): Hash32 {
  const bytes = typeof publicKey === "string" ? hexToBytes(publicKey, "publicKey") : publicKey;
  return deriveAccountIdFromBytes(bytes);
}

export function accountIdFromKeyPair(keyPair: nacl.SignKeyPair): Hash32 {
  return accountIdFromPublicKey(keyPair.publicKey);
}

export function buildTransferTransaction(
  params: TransferTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  const from = requireNonZeroL2Address(params.from, "from");
  const to = requireNonZeroL2Address(params.to, "to");
  return {
    chain_id: params.chainId,
    from,
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      Transfer: {
        to,
        asset_id: toSafeNumber(toUint(params.assetId, "assetId", 32), "assetId"),
        amount: toDecimalString(toPositiveUint(params.amount, "amount", 128)),
      },
    },
  };
}

export function signTransferTransaction(
  params: TransferTransactionParams & SigningParams,
): SignedL2Transaction {
  return signTransaction(buildTransferTransaction(params), params.keyPair);
}

export function buildWithdrawTransaction(
  params: WithdrawTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  const from = requireNonZeroL2Address(params.from, "from");
  parseTonAddress(params.l1Recipient);
  return {
    chain_id: params.chainId,
    from,
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      Withdraw: {
        asset_id: toSafeNumber(toUint(params.assetId, "assetId", 32), "assetId"),
        amount: toDecimalString(toPositiveUint(params.amount, "amount", 120)),
        l1_recipient: params.l1Recipient,
      },
    },
  };
}

export function signWithdrawTransaction(
  params: WithdrawTransactionParams & SigningParams,
): SignedL2Transaction {
  return signTransaction(buildWithdrawTransaction(params), params.keyPair);
}

export function releaseAuthorizedCell(leaf: WithdrawalProofLeaf): Cell {
  requireNonZeroL2Address(leaf.l2_sender, "l2_sender");
  return RollupRootGenerated.ReleaseAuthorized.toCell(
    RollupRootGenerated.ReleaseAuthorized.create({
      withdrawalId: hashToUint256(leaf.withdrawal_id, "withdrawal_id"),
      assetId: toUint(leaf.asset_id, "asset_id", 32),
      recipient: parseTonAddress(leaf.l1_recipient),
      amount: toUint(leaf.amount, "amount", 120),
    }),
  );
}

export function withdrawalMerkleProofCell(proof: WithdrawalMerkleProof): Cell {
  const siblings = proof.siblings.map((sibling, index) =>
    hashToUint256(sibling, `siblings[${index}]`),
  );
  if (siblings.length >= 1 << 16) {
    throw new Error("withdrawal proof has too many siblings");
  }

  const groups: bigint[][] = [];
  for (let offset = 0; offset < siblings.length; offset += WITHDRAWAL_PROOF_CHUNK_MAX) {
    groups.push(siblings.slice(offset, offset + WITHDRAWAL_PROOF_CHUNK_MAX));
  }

  let next: Cell | null = null;
  for (let i = groups.length - 1; i >= 0; i -= 1) {
    next = withdrawalProofChunkCell(groups[i], next);
  }

  const builder = beginCell()
    .storeUint(toUint(proof.leaf_index, "leaf_index", 64), 64)
    .storeUint(siblings.length, 16);
  if (next) {
    builder.storeBit(true).storeRef(next);
  } else {
    builder.storeBit(false);
  }
  return builder.endCell();
}

export function buildClaimWithdrawalBody(proof: WithdrawalProofResponse): Cell {
  normalizeHash32(proof.withdrawal_root);
  const blockHeight = toUint(proof.block_height, "block_height", 64);
  const batchNo = blockHeight + 1n;
  if (batchNo >= (1n << 64n)) {
    throw new Error("batchNo exceeds uint64");
  }

  return RollupRootGenerated.RollupRoot.createCellOfClaimWithdrawal({
    batchNo,
    withdrawalId: hashToUint256(proof.leaf.withdrawal_id, "withdrawal_id"),
    withdrawalLeaf: releaseAuthorizedCell(proof.leaf),
    merkleProof: withdrawalMerkleProofCell(proof.proof),
  });
}

export function claimWithdrawalTonConnectMessage(
  params: ClaimWithdrawalTonConnectMessageParams,
): TonConnectMessage {
  const body = buildClaimWithdrawalBody(params.proof);
  parseTonAddress(params.rollupRootAddress);
  return {
    address: params.rollupRootAddress,
    amount: toDecimalString(toPositiveUint(params.amount, "amount", 120)),
    payload: body.toBoc().toString("base64"),
  };
}

export class TonL2Client {
  public readonly baseUrl: string;

  constructor(
    baseUrl: string,
    private readonly options: TonL2ClientOptions = {},
  ) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
  }

  async submitTx(tx: SignedL2Transaction): Promise<SubmitTxResponse> {
    return this.postJson("/v1/tx", tx);
  }

  async getAccount(accountId: Hash32): Promise<L2Account> {
    const address = encodeURIComponent(l2RawAddress(parseL2Address(accountId)));
    return this.getJson(`/v1/account/${address}`);
  }

  async getSampleCounter(contractId: Hash32): Promise<SampleCounterReadResponse> {
    return this.getJson(
      `/v1/sample-counter/${encodeURIComponent(l2RawAddress(parseL2Address(contractId)))}`,
    );
  }

  async getContractMethod(
    contractId: Hash32,
    method: string,
  ): Promise<ContractGetMethodResponse> {
    const address = encodeURIComponent(l2RawAddress(parseL2Address(contractId)));
    return this.getJson(`/v1/contract/${address}/get/${encodeURIComponent(method)}`);
  }

  async getExplorerAccount(accountId: Hash32): Promise<ExplorerAccountResponse> {
    const address = encodeURIComponent(l2RawAddress(parseL2Address(accountId)));
    return this.getJson(`/v1/explorer/account/${address}`);
  }

  async getExplorerAccountTransactions(
    accountId: Hash32,
    query: { limit?: number; beforeHeight?: number; beforeIndex?: number } = {},
  ): Promise<ExplorerAccountTransactionsResponse> {
    const address = encodeURIComponent(l2RawAddress(parseL2Address(accountId)));
    const params = new URLSearchParams();
    if (query.limit !== undefined) {
      params.set("limit", query.limit.toString());
    }
    if (query.beforeHeight !== undefined) {
      params.set("before_height", query.beforeHeight.toString());
    }
    if (query.beforeIndex !== undefined) {
      params.set("before_index", query.beforeIndex.toString());
    }
    const queryString = params.toString();
    const suffix = queryString ? `?${queryString}` : "";
    return this.getJson(`/v1/explorer/account/${address}/transactions${suffix}`);
  }

  async getBlock(height: number): Promise<unknown> {
    return this.getJson(`/v1/block/${height}`);
  }

  async getWithdrawalProof(withdrawalId: Hash32): Promise<WithdrawalProofResponse> {
    return this.getJson(`/v1/proof/withdrawal/${normalizeHash32(withdrawalId)}`);
  }

  async devDeposit(deposit: DepositEvent): Promise<void> {
    await this.postJson<void>("/v1/admin/deposit", deposit, { admin: true });
  }

  async requestEntFaucet(accountId: Hash32): Promise<EntFaucetResponse> {
    const account = requireNonZeroL2Address(accountId, "accountId");
    return this.postJson(
      "/v1/admin/faucet/ent",
      { account_id: l2RawAddress(account) },
      { admin: true },
    );
  }

  async adminProduceBlock(): Promise<unknown | undefined> {
    return this.postJson("/v1/admin/produce-block", {}, { admin: true });
  }

  async submitSignedTransfer(
    params: TransferTransactionParams & SigningParams,
  ): Promise<SubmitTxResponse> {
    return this.submitTx(signTransferTransaction(params));
  }

  async submitSignedWithdraw(
    params: WithdrawTransactionParams & SigningParams,
  ): Promise<SubmitTxResponse> {
    return this.submitTx(signWithdrawTransaction(params));
  }

  async submitSignedDeployContract(
    params: DeployContractTransactionParams & SigningParams,
  ): Promise<SubmitTxResponse> {
    return this.submitTx(signDeployContractTransaction(params));
  }

  async submitSignedCallContract(
    params: CallContractTransactionParams & SigningParams,
  ): Promise<SubmitTxResponse> {
    return this.submitTx(signCallContractTransaction(params));
  }

  private async getJson<T>(path: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`);
    if (!response.ok) {
      throw await apiError(response);
    }
    return response.json() as Promise<T>;
  }

  private async postJson<T>(
    path: string,
    body: unknown,
    options: { admin?: boolean } = {},
  ): Promise<T> {
    const headers: Record<string, string> = { "content-type": "application/json" };
    if (options.admin) {
      if (!this.options.adminToken) {
        throw new Error("admin token required for L2 admin API");
      }
      headers.authorization = `Bearer ${this.options.adminToken}`;
    }

    const response = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    });
    const text = await response.text();
    if (!response.ok) {
      throw apiErrorFromText(response, text);
    }
    if (!text) {
      return undefined as T;
    }
    return JSON.parse(text) as T;
  }
}

export class EntropisClient extends TonL2Client {}

export function parseTonAddress(value: string): Address {
  return Address.parse(value);
}

function withdrawalProofChunkCell(siblings: bigint[], next: Cell | null): Cell {
  const padded = [0n, 0n, 0n];
  siblings.forEach((sibling, index) => {
    padded[index] = sibling;
  });

  const builder = beginCell()
    .storeUint(siblings.length, 8)
    .storeUint(padded[0], 256)
    .storeUint(padded[1], 256)
    .storeUint(padded[2], 256);
  if (next) {
    builder.storeBit(true).storeRef(next);
  } else {
    builder.storeBit(false);
  }
  return builder.endCell();
}

function hashToUint256(value: Hash32, field: string): bigint {
  try {
    return BigInt(`0x${normalizeHash32(value)}`);
  } catch {
    throw new Error(`${field} must be a 32-byte hex string`);
  }
}

function requireNonZeroL2Address(value: string, field: string): Hash32 {
  const parsed = parseL2Address(value);
  if (isL2ZeroAddress(parsed)) {
    throw new Error(`${field} cannot be the reserved zero address`);
  }
  return parsed;
}

function toUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = parseUint(value, field);
  if (parsed >= (1n << BigInt(bits))) {
    throw new Error(`${field} exceeds uint${bits}`);
  }
  return parsed;
}

function toPositiveUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = toUint(value, field, bits);
  if (parsed === 0n) {
    throw new Error(`${field} must be non-zero`);
  }
  return parsed;
}

function parseUint(value: UIntLike, field: string): bigint {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new Error(`${field} must be non-negative`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${field} must be a non-negative safe integer`);
    }
    return BigInt(value);
  }
  if (!/^[0-9]+$/.test(value)) {
    throw new Error(`${field} must be an unsigned decimal integer`);
  }
  return BigInt(value);
}

function toSafeNumber(value: bigint, field: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${field} exceeds JavaScript safe integer range`);
  }
  return Number(value);
}

function toDecimalString(value: bigint): string {
  return value.toString(10);
}

function hexToBytes(value: string, field: string): Uint8Array {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]+$/.test(cleaned) || cleaned.length % 2 !== 0) {
    throw new Error(`${field} must be hex`);
  }
  return Buffer.from(cleaned, "hex");
}

async function apiError(response: Response): Promise<EntropisApiError> {
  return apiErrorFromText(response, await response.text());
}

function apiErrorFromText(response: Response, text: string): EntropisApiError {
  let publicMessage = text || response.statusText || "request failed";
  try {
    const parsed = JSON.parse(text) as { error?: unknown };
    if (typeof parsed.error === "string" && parsed.error.length > 0) {
      publicMessage = parsed.error;
    }
  } catch {
    // Keep non-JSON provider or proxy text as the public message.
  }
  return new EntropisApiError(response.status, response.statusText, text, publicMessage);
}
