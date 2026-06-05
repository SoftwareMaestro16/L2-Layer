import { z } from "zod"

export const FAUCET_AMOUNT = 100

export const addressSchema = z
  .string()
  .trim()
  .min(12, "Enter a longer Entropis L2 account id.")
  .max(96, "Account id is too long.")

export type PublicConfig = {
  amountEnt: number
  batchIntervalMs: number
  cooldownSeconds: number
  enforceCooldown: boolean
  maxBatchSize: number
  githubConfigured: boolean
  nodeConfigured: boolean
}

export type FaucetUser = {
  id: number
  login: string
  avatarUrl: string | null
}

export type FaucetSession = {
  authenticated: boolean
  user: FaucetUser | null
  config: PublicConfig
}

export type FaucetClaim = {
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
  session: FaucetSession
  claims: FaucetClaim[]
}

export type FaucetBatch = {
  batchId: string
  claimIds: string[]
  status: "submitted" | "failed" | "partial"
  createdAt: number
  completedAt: number | null
  error: string | null
}

export async function fetchFaucetStatus(): Promise<FaucetStatus> {
  return fetchJson("/api/faucet/status")
}

export async function fetchFaucetBatches(): Promise<{ batches: FaucetBatch[] }> {
  return fetchJson("/api/faucet/batches")
}

export async function submitFaucetClaim(accountId: string): Promise<{ claim: FaucetClaim; duplicate: boolean }> {
  return fetchJson("/api/faucet/claim", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ account_id: accountId }),
  })
}

export async function logoutFaucet(): Promise<void> {
  await fetchJson("/api/auth/logout", { method: "POST" })
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init)
  const body = (await response.json()) as unknown
  if (!response.ok) {
    const error = isErrorBody(body) ? body.error : "request_failed"
    throw new Error(error)
  }
  return body as T
}

function isErrorBody(value: unknown): value is { error: string } {
  return typeof value === "object" && value !== null && "error" in value && typeof value.error === "string"
}
