import { isL2ZeroAddress, l2RawAddress, parseL2Address, type Hash32 } from "./address.js";
import {
  TonL2Client,
  type DepositEvent,
  type EntFaucetBatchClaimRequest,
  type EntFaucetBatchResponse,
  type EntFaucetResponse,
} from "./index.js";

export interface EntropisAdminClientOptions {
  adminToken: string;
}

export class EntropisAdminClient extends TonL2Client {
  private readonly adminToken: string;

  constructor(baseUrl: string, options: EntropisAdminClientOptions) {
    if (!options.adminToken) {
      throw new Error("admin token required for L2 admin API");
    }
    super(baseUrl);
    this.adminToken = options.adminToken;
  }

  async devDeposit(deposit: DepositEvent): Promise<void> {
    await this.postJson<void>("/v1/admin/deposit", deposit, this.authHeaders());
  }

  async requestEntFaucet(accountId: Hash32): Promise<EntFaucetResponse> {
    const account = requireNonZeroL2Address(accountId, "accountId");
    return this.postJson(
      "/v1/admin/faucet/ent",
      { account_id: l2RawAddress(account) },
      this.authHeaders(),
    );
  }

  async requestEntFaucetBatch(
    claims: EntFaucetBatchClaimRequest[],
  ): Promise<EntFaucetBatchResponse> {
    return this.postJson(
      "/v1/admin/faucet/ent/batch",
      {
        claims: claims.map((claim) => {
          if (!claim.claimId) {
            throw new Error("claimId required");
          }
          const account = requireNonZeroL2Address(claim.accountId, "accountId");
          return {
            claim_id: claim.claimId,
            account_id: l2RawAddress(account),
            amount_ent:
              claim.amountEnt === undefined ? undefined : claim.amountEnt.toString(),
          };
        }),
      },
      this.authHeaders(),
    );
  }

  async produceBlock(): Promise<unknown | undefined> {
    return this.postJson("/v1/admin/produce-block", {}, this.authHeaders());
  }

  private authHeaders(): Record<string, string> {
    return { authorization: `Bearer ${this.adminToken}` };
  }
}

function requireNonZeroL2Address(value: string, field: string): Hash32 {
  const parsed = parseL2Address(value);
  if (isL2ZeroAddress(parsed)) {
    throw new Error(`${field} cannot be the reserved zero address`);
  }
  return parsed;
}
