import type { FaucetConfig } from "./config.js";
import { l2RawAddress } from "./address.js";
import type { NodeClaimInput, NodeClaimResult } from "./types.js";

type BatchResponse = {
  claims?: Array<{
    claim_id?: string;
    faucet?: {
      deposit_id?: string;
      granted?: boolean;
    };
  }>;
};

type SingleResponse = {
  deposit_id?: string;
  granted?: boolean;
};

export class EntropisNodeClient {
  private readonly apiUrl: string;

  constructor(
    private readonly config: FaucetConfig,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {
    this.apiUrl = config.entropisApiUrl.replace(/\/+$/u, "");
  }

  async submitClaims(claims: NodeClaimInput[]): Promise<NodeClaimResult[]> {
    if (!this.config.l2AdminToken) {
      throw new Error("node_admin_not_configured");
    }
    if (claims.length === 0) {
      return [];
    }

    const batch = await this.tryBatchEndpoint(claims);
    if (batch) {
      return batch;
    }
    return this.submitClaimsIndividually(claims);
  }

  private async tryBatchEndpoint(claims: NodeClaimInput[]): Promise<NodeClaimResult[] | null> {
    const response = await this.fetchImpl(`${this.apiUrl}/v1/admin/faucet/ent/batch`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({
        claims: claims.map((claim) => ({
          claim_id: claim.claimId,
          account_id: l2RawAddress(claim.accountId),
        })),
      }),
    });
    if (response.status === 404 || response.status === 405) {
      return null;
    }
    if (!response.ok) {
      throw new Error(safeNodeError(response.status));
    }
    const body = (await response.json()) as BatchResponse;
    const byClaim = new Map((body.claims ?? []).map((claim) => [claim.claim_id, claim]));
    return claims.map((claim) => {
      const result = byClaim.get(claim.claimId);
      if (!result?.faucet) {
        return failedResult(claim, "node_batch_missing_claim");
      }
      return {
        claimId: claim.claimId,
        status: result.faucet.granted ? "granted" : "duplicate",
        depositId: result.faucet.deposit_id ?? null,
        error: null,
      };
    });
  }

  private async submitClaimsIndividually(claims: NodeClaimInput[]): Promise<NodeClaimResult[]> {
    const out: NodeClaimResult[] = [];
    for (const claim of claims) {
      try {
        const response = await this.fetchImpl(`${this.apiUrl}/v1/admin/faucet/ent`, {
          method: "POST",
          headers: this.headers(),
          body: JSON.stringify({ account_id: l2RawAddress(claim.accountId) }),
        });
        if (!response.ok) {
          out.push(failedResult(claim, safeNodeError(response.status)));
          continue;
        }
        const body = (await response.json()) as SingleResponse;
        out.push({
          claimId: claim.claimId,
          status: body.granted ? "granted" : "duplicate",
          depositId: body.deposit_id ?? null,
          error: null,
        });
      } catch {
        out.push(failedResult(claim, "node_request_failed"));
      }
    }
    return out;
  }

  private headers(): HeadersInit {
    return {
      authorization: `Bearer ${this.config.l2AdminToken ?? ""}`,
      "content-type": "application/json",
    };
  }
}

function failedResult(claim: NodeClaimInput, reason: string): NodeClaimResult {
  return {
    claimId: claim.claimId,
    status: "failed",
    depositId: null,
    error: reason,
  };
}

function safeNodeError(status: number): string {
  if (status === 401 || status === 403) {
    return "node_admin_rejected";
  }
  if (status === 400 || status === 409) {
    return "node_claim_rejected";
  }
  if (status === 429) {
    return "node_rate_limited";
  }
  return "node_unavailable";
}
