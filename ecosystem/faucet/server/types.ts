export type GitHubUser = {
  id: number
  login: string
  avatarUrl: string | null
}

export type FaucetConfig = {
  entropisApiUrl: string
  l2AdminToken: string | null
  githubClientId: string | null
  githubClientSecret: string | null
  amountEnt: number
  batchIntervalMs: number
  cooldownSeconds: number
  enforceCooldown: boolean
  maxBatchSize: number
  host: string
  port: number
  sessionCookieName: string
  sessionTtlMs: number
  rateLimitWindowMs: number
  rateLimitMax: number
}

export type FaucetClaim = {
  claimId: string
  githubUserId: number
  githubLogin: string
  accountId: string
  accountRawAddress: string
  amountEnt: number
  status: ClaimStatus
  createdAt: number
  updatedAt: number
  attempts: number
  lastError: string | null
  nodeDepositId: string | null
}

export type ClaimStatus = "pending" | "processing" | "granted" | "duplicate" | "failed"

export type NodeClaimResult = {
  claimId: string
  status: "granted" | "duplicate" | "failed"
  depositId: string | null
  error: string | null
}
