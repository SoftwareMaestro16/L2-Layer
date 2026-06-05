import type { IncomingMessage, ServerResponse } from "node:http";
import { AddressError, normalizeL2Address, rejectZeroAddress } from "./address.js";
import type { FaucetConfig } from "./config.js";
import { publicConfig } from "./config.js";
import type { GitHubOAuthClient } from "./github.js";
import {
  clearCookie,
  clientIp,
  cookieValue,
  json,
  methodNotAllowed,
  readJson,
  redirect,
  requestOrigin,
  serveStatic,
  setCookie,
} from "./http.js";
import type { FaucetStore } from "./store.js";
import type { FaucetClaim, Session } from "./types.js";

type AppDeps = {
  config: FaucetConfig;
  store: FaucetStore;
  githubClient: GitHubOAuthClient | null;
};

export function createFaucetApp(deps: AppDeps) {
  return async (request: IncomingMessage, response: ServerResponse) => {
    const url = new URL(request.url ?? "/", requestOrigin(request));
    try {
      if (url.pathname === "/api/auth/github/start") return githubStart(deps, request, response);
      if (url.pathname === "/api/auth/github/callback") return githubCallback(deps, request, url, response);
      if (url.pathname === "/api/auth/logout") return logout(deps, request, response);
      if (url.pathname === "/api/session") return session(deps, request, response);
      if (url.pathname === "/api/faucet/claim") return claim(deps, request, response);
      if (url.pathname === "/api/faucet/status") return status(deps, request, response);
      if (url.pathname === "/api/faucet/batches") return batches(deps, response);
      if (url.pathname.startsWith("/api/")) return json(response, 404, { error: "not_found" });
      if (await serveStatic(response, url.pathname)) return;
      if (await serveStatic(response, "/index.html")) return;
      json(response, 404, { error: "not_found" });
    } catch (error) {
      handleError(response, error);
    }
  };
}

function githubStart(deps: AppDeps, request: IncomingMessage, response: ServerResponse): void {
  if (request.method !== "GET") return methodNotAllowed(response);
  if (!rateLimit(deps, request, "oauth_start")) return rateLimited(response);
  if (!deps.githubClient) return json(response, 503, { error: "github_oauth_not_configured" });
  const redirectUri = `${requestOrigin(request)}/api/auth/github/callback`;
  const oauth = deps.store.createOAuthState(redirectUri);
  redirect(response, deps.githubClient.authorizationUrl(oauth));
}

async function githubCallback(
  deps: AppDeps,
  request: IncomingMessage,
  url: URL,
  response: ServerResponse,
): Promise<void> {
  if (!rateLimit(deps, request, "oauth_callback")) return rateLimited(response);
  if (!deps.githubClient) return json(response, 503, { error: "github_oauth_not_configured" });
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  if (!code || !state) return redirect(response, "/?auth=failed");
  const oauth = deps.store.consumeOAuthState(state);
  if (!oauth) return redirect(response, "/?auth=failed");
  const user = await deps.githubClient.completeCallback({ code, redirectUri: oauth.redirectUri });
  const session = deps.store.createSession(user, deps.config.sessionTtlMs);
  redirect(response, "/", {
    "set-cookie": setCookie(
      deps.config.sessionCookieName,
      session.id,
      Math.floor(deps.config.sessionTtlMs / 1000),
    ),
  });
}

function logout(deps: AppDeps, request: IncomingMessage, response: ServerResponse): void {
  if (request.method !== "POST") return methodNotAllowed(response);
  deps.store.deleteSession(cookieValue(request, deps.config.sessionCookieName));
  json(response, 200, { ok: true }, { "set-cookie": clearCookie(deps.config.sessionCookieName) });
}

function session(deps: AppDeps, request: IncomingMessage, response: ServerResponse): void {
  if (request.method !== "GET") return methodNotAllowed(response);
  json(response, 200, sessionBody(deps, currentSession(deps, request)));
}

async function claim(deps: AppDeps, request: IncomingMessage, response: ServerResponse): Promise<void> {
  if (request.method !== "POST") return methodNotAllowed(response);
  if (!rateLimit(deps, request, "claim")) return rateLimited(response);
  const active = currentSession(deps, request);
  if (!active) return json(response, 401, { error: "github_session_required" });
  const body = await readJson<{ account_id?: unknown }>(request);
  if (typeof body.account_id !== "string") {
    return json(response, 400, { error: "account_id_required" });
  }
  const accountId = normalizeL2Address(body.account_id);
  rejectZeroAddress(accountId);
  const result = deps.store.createClaim({
    user: active.user,
    accountId,
    amountEnt: deps.config.amountEnt,
    cooldownSeconds: deps.config.cooldownSeconds,
    enforceCooldown: deps.config.enforceCooldown,
  });
  if (!result.ok && result.code === "cooldown") {
    return json(response, 429, { error: "cooldown", retryAt: result.retryAt });
  }
  if (!result.ok) {
    return json(response, 202, { claim: claimBody(result.claim), duplicate: true });
  }
  json(response, 202, { claim: claimBody(result.claim), duplicate: false });
}

function status(deps: AppDeps, request: IncomingMessage, response: ServerResponse): void {
  if (request.method !== "GET") return methodNotAllowed(response);
  const active = currentSession(deps, request);
  json(response, 200, {
    config: publicConfig(deps.config),
    pendingCount: deps.store.pendingCount(),
    session: sessionBody(deps, active),
    claims: active ? deps.store.sessionClaims(active.user.id).map(claimBody) : [],
  });
}

function batches(deps: AppDeps, response: ServerResponse): void {
  json(response, 200, { batches: deps.store.safeBatches() });
}

function currentSession(deps: AppDeps, request: IncomingMessage): Session | null {
  return deps.store.getSession(cookieValue(request, deps.config.sessionCookieName));
}

function sessionBody(deps: AppDeps, active: Session | null) {
  return {
    authenticated: Boolean(active),
    user: active
      ? { id: active.user.id, login: active.user.login, avatarUrl: active.user.avatarUrl }
      : null,
    config: publicConfig(deps.config),
  };
}

function claimBody(claim: FaucetClaim) {
  return {
    claimId: claim.claimId,
    accountId: claim.accountId,
    accountRawAddress: claim.accountRawAddress,
    amountEnt: claim.amountEnt,
    status: claim.status,
    createdAt: claim.createdAt,
    updatedAt: claim.updatedAt,
    attempts: claim.attempts,
    lastError: claim.lastError,
    nodeDepositId: claim.nodeDepositId,
  };
}

function rateLimit(deps: AppDeps, request: IncomingMessage, action: string): boolean {
  return deps.store.checkRateLimit(
    `${action}:${clientIp(request)}`,
    deps.config.rateLimitMax,
    deps.config.rateLimitWindowMs,
  );
}

function rateLimited(response: ServerResponse): void {
  json(response, 429, { error: "rate_limited" });
}

function handleError(response: ServerResponse, error: unknown): void {
  if (error instanceof AddressError) {
    return json(response, 400, { error: error.message });
  }
  if (error instanceof SyntaxError) {
    return json(response, 400, { error: "invalid_json" });
  }
  if (error instanceof Error && error.message === "request_too_large") {
    return json(response, 413, { error: "request_too_large" });
  }
  json(response, 500, { error: "faucet_internal_error" });
}
