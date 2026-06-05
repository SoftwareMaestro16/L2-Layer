import { l2RawAddress } from "./address.js"
import type { FaucetConfig, NodeClaimResult } from "./types.js"

type ClaimInput = {
  claimId: string
  accountId: string
}

export class EntropisNodeClient {
  private readonly apiUrl: string

  constructor(
    private readonly config: FaucetConfig,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {
    this.apiUrl = config.entropisApiUrl.replace(/\/+$/u, "")
  }

  async submitClaims(claims: ClaimInput[]): Promise<NodeClaimResult[]> {
    if (!this.config.l2AdminToken) {
      throw new Error("node_admin_not_configured")
    }
    if (claims.length === 0) return []

    const batch = await this.tryBatchEndpoint(claims)
    if (batch) return batch

    return this.submitClaimsIndividually(claims)
  }

  private async tryBatchEndpoint(claims: ClaimInput[]) {
    const response = await this.fetchImpl(`${this.apiUrl}/v1/admin/faucet/ent/batch`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({
        claims: claims.map((claim) => ({
          claim_id: claim.claimId,
          account_id: l2RawAddress(claim.accountId),
        })),
      }),
    })

    if (response.status === 404 || response.status === 405) return null
    if (!response.ok) throw new Error(safeNodeError(response.status))

    const body = (await response.json()) as { claims?: NodeBatchClaim[] }
    const byClaim = new Map((body.claims ?? []).map((claim) => [claim.claim_id, claim]))

    return claims.map((claim) => {
      const result = byClaim.get(claim.claimId)
      if (!result?.faucet) {
        if (!result) return failedResult(claim, "node_batch_missing_claim")
        return {
          claimId: claim.claimId,
          status: statusFromNode(result.status),
          depositId: result.deposit_id ?? null,
          error: result.error_code ?? null,
        }
      }

      return {
        claimId: claim.claimId,
        status: result.faucet.granted ? "granted" : "duplicate",
        depositId: result.faucet.deposit_id ?? null,
        error: null,
      } satisfies NodeClaimResult
    })
  }

  private async submitClaimsIndividually(claims: ClaimInput[]) {
    const out: NodeClaimResult[] = []

    for (const claim of claims) {
      try {
        const response = await this.fetchImpl(`${this.apiUrl}/v1/admin/faucet/ent`, {
          method: "POST",
          headers: this.headers(),
          body: JSON.stringify({ account_id: l2RawAddress(claim.accountId) }),
        })

        if (!response.ok) {
          out.push(failedResult(claim, safeNodeError(response.status)))
          continue
        }

        const body = (await response.json()) as { granted?: boolean; deposit_id?: string | null }
        out.push({
          claimId: claim.claimId,
          status: body.granted ? "granted" : "duplicate",
          depositId: body.deposit_id ?? null,
          error: null,
        })
      } catch {
        out.push(failedResult(claim, "node_request_failed"))
      }
    }

    return out
  }

  private headers() {
    return {
      authorization: `Bearer ${this.config.l2AdminToken ?? ""}`,
      "content-type": "application/json",
    }
  }
}

type NodeBatchClaim = {
  claim_id: string
  status?: string
  deposit_id?: string | null
  error_code?: string | null
  faucet?: {
    granted: boolean
    deposit_id?: string | null
  }
}

function failedResult(claim: ClaimInput, reason: string): NodeClaimResult {
  return {
    claimId: claim.claimId,
    status: "failed",
    depositId: null,
    error: reason,
  }
}

function safeNodeError(status: number) {
  if (status === 401 || status === 403) return "node_admin_rejected"
  if (status === 400 || status === 409) return "node_claim_rejected"
  if (status === 429) return "node_rate_limited"
  return "node_unavailable"
}

function statusFromNode(status: string | undefined): NodeClaimResult["status"] {
  if (status === "granted") return "granted"
  if (status === "duplicate_claim" || status === "duplicate_account") return "duplicate"
  return "failed"
}
