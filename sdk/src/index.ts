import { Address, beginCell, Cell } from "@ton/core";
import nacl from "tweetnacl";
import { signingPayload } from "./consensus.js";
import * as RollupRootGenerated from "./generated/RollupRoot.gen.js";

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

export type Hash32 = string;

export const L2_NATIVE_GAS_ASSET = 0;
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

export interface TonL2ClientOptions {
  adminToken?: string;
}

export function normalizeHash32(value: string): Hash32 {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return cleaned.toLowerCase();
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

export function buildWithdrawTransaction(
  params: WithdrawTransactionParams,
): Omit<SignedL2Transaction, "public_key" | "signature"> {
  parseTonAddress(params.l1Recipient);
  return {
    chain_id: params.chainId,
    from: normalizeHash32(params.from),
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

export function tonDepositForwardPayload(l2Recipient: Hash32) {
  const recipient = BigInt(`0x${normalizeHash32(l2Recipient)}`);
  return beginCell().storeUint(recipient, 256).endCell();
}

export function jettonDepositForwardPayload(l2Recipient: Hash32) {
  return tonDepositForwardPayload(l2Recipient);
}

export function encodeDepositTonBody(queryId: bigint, amount: bigint, l2Recipient: Hash32) {
  return beginCell()
    .storeUint(0x4c324405, 32)
    .storeUint(queryId, 64)
    .storeCoins(amount)
    .storeUint(BigInt(`0x${normalizeHash32(l2Recipient)}`), 256)
    .endCell();
}

export function releaseAuthorizedCell(leaf: WithdrawalProofLeaf): Cell {
  normalizeHash32(leaf.l2_sender);
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
  constructor(
    public readonly baseUrl: string,
    private readonly options: TonL2ClientOptions = {},
  ) {}

  async submitTx(tx: SignedL2Transaction): Promise<SubmitTxResponse> {
    return this.postJson("/v1/tx", tx);
  }

  async getAccount(accountId: Hash32): Promise<unknown> {
    return this.getJson(`/v1/account/${normalizeHash32(accountId)}`);
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

  private async getJson<T>(path: string): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`);
    if (!response.ok) {
      throw new Error(`L2 API error ${response.status}: ${await response.text()}`);
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
      throw new Error(`L2 API error ${response.status}: ${text}`);
    }
    if (!text) {
      return undefined as T;
    }
    return JSON.parse(text) as T;
  }
}

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
