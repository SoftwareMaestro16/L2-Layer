import { signTransferTransaction, signWithdrawTransaction } from "./transactions.js";
import type {
  DepositEvent,
  EntFaucetResponse,
  Hash32,
  L2Account,
  SignedL2Transaction,
  SubmitTxResponse,
  TonL2ClientOptions,
  TransferTransactionParams,
  WithdrawalProofResponse,
  WithdrawTransactionParams,
  SigningParams,
} from "./types.js";
import { normalizeHash32 } from "./validation.js";
import { apiError, apiErrorFromText, EntropisApiError } from "./api-error.js";

export { EntropisApiError };

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

  async requestEntFaucet(accountId: Hash32): Promise<EntFaucetResponse> {
    return this.postJson(
      "/v1/admin/faucet/ent",
      { account_id: normalizeHash32(accountId) },
      { admin: true },
    );
  }

  async submitSignedTransfer(params: TransferTransactionParams & SigningParams) {
    return this.submitTx(signTransferTransaction(params));
  }

  async submitSignedWithdraw(params: WithdrawTransactionParams & SigningParams) {
    return this.submitTx(signWithdrawTransaction(params));
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
