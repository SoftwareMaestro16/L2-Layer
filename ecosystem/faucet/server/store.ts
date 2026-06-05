import { randomBytes } from "node:crypto"

import { l2RawAddress } from "./address.js"
import { createPkcePair } from "./github.js"
import type { FaucetClaim, GitHubUser, NodeClaimResult } from "./types.js"

const MAX_HISTORY = 100

type Session = {
  id: string
  user: GitHubUser
  expiresAt: number
}

type OAuthState = {
  state: string
  redirectUri: string
  codeVerifier: string
  codeChallenge: string
  expiresAt: number
}

type Batch = {
  batchId: string
  claimIds: string[]
  status: "submitted" | "partial" | "failed"
  createdAt: number
  completedAt: number | null
  error: string | null
}

export class FaucetStore {
  private readonly sessions = new Map<string, Session>()
  private readonly oauthStates = new Map<string, OAuthState>()
  private readonly claims = new Map<string, FaucetClaim>()
  private readonly pending: string[] = []
  private readonly processing = new Set<string>()
  private readonly cooldowns = new Map<string, number>()
  private readonly batches: Batch[] = []
  private readonly rateLimits = new Map<string, { count: number; resetAt: number }>()

  constructor(private readonly now: () => number = () => Date.now()) {}

  createOAuthState(redirectUri: string) {
    const state = randomHex(16)
    const pkce = createPkcePair()
    const entry = {
      state,
      redirectUri,
      codeVerifier: pkce.codeVerifier,
      codeChallenge: pkce.codeChallenge,
      expiresAt: this.now() + 10 * 60_000,
    }
    this.oauthStates.set(state, entry)
    return entry
  }

  consumeOAuthState(state: string) {
    const entry = this.oauthStates.get(state) ?? null
    this.oauthStates.delete(state)
    if (!entry || entry.expiresAt < this.now()) {
      return null
    }
    return entry
  }

  createSession(user: GitHubUser, ttlMs: number) {
    const session = { id: randomHex(32), user, expiresAt: this.now() + ttlMs }
    this.sessions.set(session.id, session)
    return session
  }

  getSession(id: string | null) {
    if (!id) return null

    const session = this.sessions.get(id) ?? null
    if (!session || session.expiresAt < this.now()) {
      if (session) this.sessions.delete(id)
      return null
    }

    return session
  }

  deleteSession(id: string | null) {
    if (id) {
      this.sessions.delete(id)
    }
  }

  createClaim(params: {
    user: GitHubUser
    accountId: string
    amountEnt: number
    cooldownSeconds: number
    enforceCooldown: boolean
  }) {
    const pending = this.pendingClaim(params.user.id, params.accountId)
    if (pending) {
      return { ok: false as const, code: "already_pending" as const, claim: pending }
    }

    if (params.enforceCooldown) {
      const retryAt = this.cooldownRetryAt(params.user.id, params.accountId)
      if (retryAt > this.now()) {
        return { ok: false as const, code: "cooldown" as const, retryAt }
      }
    }

    const claim: FaucetClaim = {
      claimId: randomHex(32),
      githubUserId: params.user.id,
      githubLogin: params.user.login,
      accountId: params.accountId,
      accountRawAddress: l2RawAddress(params.accountId),
      amountEnt: params.amountEnt,
      status: "pending",
      createdAt: this.now(),
      updatedAt: this.now(),
      attempts: 0,
      lastError: null,
      nodeDepositId: null,
    }
    this.claims.set(claim.claimId, claim)
    this.pending.push(claim.claimId)
    this.setCooldown(params.user.id, params.accountId, params.cooldownSeconds)

    return { ok: true as const, claim }
  }

  takePending(limit: number) {
    const taken: FaucetClaim[] = []

    while (taken.length < limit && this.pending.length > 0) {
      const claimId = this.pending.shift()
      if (!claimId || this.processing.has(claimId)) continue

      const claim = this.claims.get(claimId)
      if (!claim || claim.status !== "pending") continue

      claim.status = "processing"
      claim.attempts += 1
      claim.updatedAt = this.now()
      this.processing.add(claimId)
      taken.push({ ...claim })
    }

    return taken
  }

  startBatch(claims: FaucetClaim[]) {
    const batch: Batch = {
      batchId: randomHex(16),
      claimIds: claims.map((claim) => claim.claimId),
      status: "submitted",
      createdAt: this.now(),
      completedAt: null,
      error: null,
    }
    this.batches.unshift(batch)
    this.batches.splice(MAX_HISTORY)
    return batch
  }

  completeBatch(batchId: string, results: NodeClaimResult[]) {
    const resultById = new Map(results.map((result) => [result.claimId, result]))
    const batch = this.batches.find((item) => item.batchId === batchId)
    let failed = 0

    for (const claimId of batch?.claimIds ?? []) {
      const claim = this.claims.get(claimId)
      const result = resultById.get(claimId)
      if (!claim || !result) continue

      claim.status = result.status
      claim.lastError = result.error
      claim.nodeDepositId = result.depositId
      claim.updatedAt = this.now()
      this.processing.delete(claimId)
      if (result.status === "failed") failed += 1
    }

    if (batch) {
      batch.completedAt = this.now()
      batch.status = failed === 0 ? "submitted" : failed === batch.claimIds.length ? "failed" : "partial"
      batch.error = failed > 0 ? "claim_failed" : null
    }
  }

  failBatch(batchId: string, reason: string) {
    const batch = this.batches.find((item) => item.batchId === batchId)

    for (const claimId of batch?.claimIds ?? []) {
      const claim = this.claims.get(claimId)
      if (!claim) continue

      claim.status = "pending"
      claim.lastError = reason
      claim.updatedAt = this.now()
      this.processing.delete(claimId)
      this.pending.push(claimId)
    }

    if (batch) {
      batch.status = "failed"
      batch.completedAt = this.now()
      batch.error = reason
    }
  }

  sessionClaims(githubUserId: number) {
    return Array.from(this.claims.values())
      .filter((claim) => claim.githubUserId === githubUserId)
      .sort((left, right) => right.createdAt - left.createdAt)
      .slice(0, 20)
  }

  safeBatches() {
    return this.batches.slice(0, 20)
  }

  pendingCount() {
    return this.pending.length + this.processing.size
  }

  checkRateLimit(key: string, max: number, windowMs: number) {
    const now = this.now()
    const bucket = this.rateLimits.get(key)

    if (!bucket || bucket.resetAt <= now) {
      this.rateLimits.set(key, { count: 1, resetAt: now + windowMs })
      return true
    }

    bucket.count += 1
    return bucket.count <= max
  }

  private pendingClaim(githubUserId: number, accountId: string) {
    return (
      Array.from(this.claims.values()).find(
        (claim) =>
          claim.githubUserId === githubUserId &&
          claim.accountId === accountId &&
          (claim.status === "pending" || claim.status === "processing"),
      ) ?? null
    )
  }

  private cooldownRetryAt(githubUserId: number, accountId: string) {
    return Math.max(
      this.cooldowns.get(`github:${githubUserId}`) ?? 0,
      this.cooldowns.get(`account:${accountId}`) ?? 0,
    )
  }

  private setCooldown(githubUserId: number, accountId: string, cooldownSeconds: number) {
    const retryAt = this.now() + cooldownSeconds * 1000
    this.cooldowns.set(`github:${githubUserId}`, retryAt)
    this.cooldowns.set(`account:${accountId}`, retryAt)
  }
}

function randomHex(bytes: number) {
  return randomBytes(bytes).toString("hex")
}
