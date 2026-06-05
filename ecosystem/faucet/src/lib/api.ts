export type PublicConfig = {
  amountEnt: number
  batchIntervalMs: number
  cooldownSeconds: number
  enforceCooldown: boolean
  maxBatchSize: number
  githubConfigured: boolean
  nodeConfigured: boolean
}

export type SessionUser = {
  id: number
  login: string
  avatarUrl: string | null
}

export type Claim = {
  claimId: string
  accountId: string
  accountRawAddress: string
  amountEnt: number
  status: "pending" | "processing" | "granted" | "duplicate" | "failed"
  createdAt: number
  updatedAt: number
  attempts: number
  lastError: string | null
  nodeDepositId: string | null
}

export type FaucetStatus = {
  config: PublicConfig
  pendingCount: number
  session: {
    authenticated: boolean
    user: SessionUser | null
    config: PublicConfig
  }
  claims: Claim[]
}

export async function fetchStatus() {
  return request<FaucetStatus>("/api/faucet/status")
}

export async function claimEnt(accountId: string) {
  return request<{ claim: Claim; duplicate: boolean }>("/api/faucet/claim", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ account_id: accountId }),
  })
}

export async function logout() {
  return request<{ ok: boolean }>("/api/auth/logout", { method: "POST" })
}

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    credentials: "same-origin",
    ...init,
  })
  const body = (await response.json()) as T | { error?: string }

  if (!response.ok) {
    const error = isErrorBody(body) ? body.error : null
    throw new Error(error ?? "request_failed")
  }

  return body as T
}

function isErrorBody(value: unknown): value is { error: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "error" in value &&
    typeof (value as { error?: unknown }).error === "string"
  )
}
