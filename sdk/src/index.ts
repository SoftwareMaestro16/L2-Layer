import { Address, beginCell } from "@ton/core";
import nacl from "tweetnacl";
import { signingPayload } from "./consensus.js";

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

export function tonDepositForwardPayload(l2Recipient: Hash32) {
  const recipient = BigInt(`0x${normalizeHash32(l2Recipient)}`);
  return beginCell().storeUint(recipient, 256).endCell();
}

export function encodeDepositTonBody(queryId: bigint, amount: bigint, l2Recipient: Hash32) {
  return beginCell()
    .storeUint(0x4c324405, 32)
    .storeUint(queryId, 64)
    .storeCoins(amount)
    .storeUint(BigInt(`0x${normalizeHash32(l2Recipient)}`), 256)
    .endCell();
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

  async getWithdrawalProof(withdrawalId: Hash32): Promise<unknown> {
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
