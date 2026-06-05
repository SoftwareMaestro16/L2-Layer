import { randomBytes } from "node:crypto";
import { l2RawAddress } from "./address.js";
import type { FaucetBatch, FaucetClaim, GitHubUser, NodeClaimResult, OAuthState, Session } from "./types.js";

const MAX_HISTORY = 100;

export type ClaimCreateResult =
  | { ok: true; claim: FaucetClaim }
  | { ok: false; code: "cooldown"; retryAt: number }
  | { ok: false; code: "already_pending"; claim: FaucetClaim };

export class FaucetStore {
  private sessions = new Map<string, Session>();
  private oauthStates = new Map<string, OAuthState>();
  private claims = new Map<string, FaucetClaim>();
  private pending: string[] = [];
  private processing = new Set<string>();
  private cooldowns = new Map<string, number>();
  private batches: FaucetBatch[] = [];
  private rateLimits = new Map<string, { count: number; resetAt: number }>();

  constructor(private readonly now = () => Date.now()) {}

  createOAuthState(redirectUri: string): OAuthState {
    const state = randomHex(16);
    const entry = { state, redirectUri, expiresAt: this.now() + 10 * 60_000 };
    this.oauthStates.set(state, entry);
    return entry;
  }

  consumeOAuthState(state: string): OAuthState | null {
    const entry = this.oauthStates.get(state) ?? null;
    this.oauthStates.delete(state);
    if (!entry || entry.expiresAt < this.now()) {
      return null;
    }
    return entry;
  }

  createSession(user: GitHubUser, ttlMs: number): Session {
    const session = { id: randomHex(32), user, expiresAt: this.now() + ttlMs };
    this.sessions.set(session.id, session);
    return session;
  }

  getSession(id: string | null): Session | null {
    if (!id) {
      return null;
    }
    const session = this.sessions.get(id) ?? null;
    if (!session || session.expiresAt < this.now()) {
      if (session) this.sessions.delete(id);
      return null;
    }
    return session;
  }

  deleteSession(id: string | null): void {
    if (id) {
      this.sessions.delete(id);
    }
  }

  createClaim(params: {
    user: GitHubUser;
    accountId: string;
    amountEnt: number;
    cooldownSeconds: number;
    enforceCooldown: boolean;
  }): ClaimCreateResult {
    const pending = this.pendingClaim(params.user.id, params.accountId);
    if (pending) {
      return { ok: false, code: "already_pending", claim: pending };
    }
    if (params.enforceCooldown) {
      const retryAt = this.cooldownRetryAt(params.user.id, params.accountId);
      if (retryAt > this.now()) {
        return { ok: false, code: "cooldown", retryAt };
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
    };
    this.claims.set(claim.claimId, claim);
    this.pending.push(claim.claimId);
    this.setCooldown(params.user.id, params.accountId, params.cooldownSeconds);
    return { ok: true, claim };
  }

  takePending(limit: number): FaucetClaim[] {
    const taken: FaucetClaim[] = [];
    while (taken.length < limit && this.pending.length > 0) {
      const claimId = this.pending.shift();
      if (!claimId || this.processing.has(claimId)) continue;
      const claim = this.claims.get(claimId);
      if (!claim || claim.status !== "pending") continue;
      claim.status = "processing";
      claim.attempts += 1;
      claim.updatedAt = this.now();
      this.processing.add(claimId);
      taken.push({ ...claim });
    }
    return taken;
  }

  startBatch(claims: FaucetClaim[]): FaucetBatch {
    const batch: FaucetBatch = {
      batchId: randomHex(16),
      claimIds: claims.map((claim) => claim.claimId),
      status: "submitted",
      createdAt: this.now(),
      completedAt: null,
      error: null,
    };
    this.batches.unshift(batch);
    this.batches.splice(MAX_HISTORY);
    return batch;
  }

  completeBatch(batchId: string, results: NodeClaimResult[]): void {
    const resultById = new Map(results.map((result) => [result.claimId, result]));
    const batch = this.batches.find((item) => item.batchId === batchId);
    let failed = 0;
    for (const claimId of batch?.claimIds ?? []) {
      const claim = this.claims.get(claimId);
      const result = resultById.get(claimId);
      if (!claim || !result) continue;
      claim.status = result.status;
      claim.lastError = result.error;
      claim.nodeDepositId = result.depositId;
      claim.updatedAt = this.now();
      this.processing.delete(claimId);
      if (result.status === "failed") failed += 1;
    }
    if (batch) {
      batch.completedAt = this.now();
      batch.status = failed === 0 ? "submitted" : failed === batch.claimIds.length ? "failed" : "partial";
      batch.error = failed > 0 ? "claim_failed" : null;
    }
  }

  failBatch(batchId: string, reason: string): void {
    const batch = this.batches.find((item) => item.batchId === batchId);
    for (const claimId of batch?.claimIds ?? []) {
      const claim = this.claims.get(claimId);
      if (!claim) continue;
      claim.status = "pending";
      claim.lastError = reason;
      claim.updatedAt = this.now();
      this.processing.delete(claimId);
      this.pending.push(claimId);
    }
    if (batch) {
      batch.status = "failed";
      batch.completedAt = this.now();
      batch.error = reason;
    }
  }

  sessionClaims(githubUserId: number): FaucetClaim[] {
    return Array.from(this.claims.values())
      .filter((claim) => claim.githubUserId === githubUserId)
      .sort((left, right) => right.createdAt - left.createdAt)
      .slice(0, 20);
  }

  safeBatches(): FaucetBatch[] {
    return this.batches.slice(0, 20);
  }

  pendingCount(): number {
    return this.pending.length + this.processing.size;
  }

  checkRateLimit(key: string, max: number, windowMs: number): boolean {
    const now = this.now();
    const bucket = this.rateLimits.get(key);
    if (!bucket || bucket.resetAt <= now) {
      this.rateLimits.set(key, { count: 1, resetAt: now + windowMs });
      return true;
    }
    bucket.count += 1;
    return bucket.count <= max;
  }

  private pendingClaim(githubUserId: number, accountId: string): FaucetClaim | null {
    return (
      Array.from(this.claims.values()).find(
        (claim) =>
          claim.githubUserId === githubUserId &&
          claim.accountId === accountId &&
          (claim.status === "pending" || claim.status === "processing"),
      ) ?? null
    );
  }

  private cooldownRetryAt(githubUserId: number, accountId: string): number {
    return Math.max(
      this.cooldowns.get(`github:${githubUserId}`) ?? 0,
      this.cooldowns.get(`account:${accountId}`) ?? 0,
    );
  }

  private setCooldown(githubUserId: number, accountId: string, cooldownSeconds: number): void {
    const retryAt = this.now() + cooldownSeconds * 1000;
    this.cooldowns.set(`github:${githubUserId}`, retryAt);
    this.cooldowns.set(`account:${accountId}`, retryAt);
  }
}

function randomHex(bytes: number): string {
  return randomBytes(bytes).toString("hex");
}
