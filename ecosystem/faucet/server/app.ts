import fastifyCookie from "@fastify/cookie"
import fastifyStatic from "@fastify/static"
import Fastify, {
  type FastifyInstance,
  type FastifyReply,
  type FastifyRequest,
} from "fastify"
import { join } from "node:path"
import { z, ZodError } from "zod"

import { AddressError, normalizeL2Address, rejectZeroAddress } from "./address.js"
import { publicConfig } from "./config.js"
import type { GitHubOAuthClient } from "./github.js"
import type { FaucetStore } from "./store.js"
import type { FaucetClaim, FaucetConfig } from "./types.js"

const claimRequestSchema = z.object({
  account_id: z.string().trim().min(1),
})

const githubCallbackSchema = z.object({
  code: z.string().trim().min(1),
  state: z.string().trim().min(1),
})

export type FaucetDeps = {
  config: FaucetConfig
  store: FaucetStore
  githubClient: GitHubOAuthClient | null
}

export async function createFaucetApp(deps: FaucetDeps) {
  const app = Fastify({
    bodyLimit: 16 * 1024,
    logger: false,
    trustProxy: true,
  })

  await app.register(fastifyCookie)
  await app.register(fastifyStatic, {
    root: join(process.cwd(), "dist"),
  })

  registerApiRoutes(app, deps)
  registerErrorHandling(app)

  app.setNotFoundHandler(async (request, reply) => {
    if (request.url.startsWith("/api/")) {
      return reply.code(404).send({ error: "not_found" })
    }

    return reply.sendFile("index.html")
  })

  return app
}

function registerApiRoutes(app: FastifyInstance, deps: FaucetDeps) {
  app.get("/api/auth/github/start", async (request, reply) => {
    if (!rateLimit(deps, request, "oauth_start")) return rateLimited(reply)
    if (!deps.githubClient) return reply.code(503).send({ error: "github_oauth_not_configured" })

    const redirectUri = `${requestOrigin(request)}/api/auth/github/callback`
    const oauth = deps.store.createOAuthState(redirectUri)
    return reply.redirect(deps.githubClient.authorizationUrl(oauth))
  })

  app.get("/api/auth/github/callback", async (request, reply) => {
    if (!rateLimit(deps, request, "oauth_callback")) return rateLimited(reply)
    if (!deps.githubClient) return reply.code(503).send({ error: "github_oauth_not_configured" })

    const parsed = githubCallbackSchema.safeParse(request.query)
    if (!parsed.success) return reply.redirect("/?auth=failed")

    const oauth = deps.store.consumeOAuthState(parsed.data.state)
    if (!oauth) return reply.redirect("/?auth=failed")

    const user = await deps.githubClient.completeCallback({
      code: parsed.data.code,
      redirectUri: oauth.redirectUri,
      codeVerifier: oauth.codeVerifier,
    })
    const active = deps.store.createSession(user, deps.config.sessionTtlMs)

    reply.setCookie(deps.config.sessionCookieName, active.id, {
      httpOnly: true,
      maxAge: Math.floor(deps.config.sessionTtlMs / 1000),
      path: "/",
      sameSite: "lax",
      secure: requestIsSecure(request),
    })
    return reply.redirect("/")
  })

  app.post("/api/auth/logout", async (request, reply) => {
    deps.store.deleteSession(cookieValue(deps, request))
    reply.clearCookie(deps.config.sessionCookieName, { path: "/" })
    return reply.send({ ok: true })
  })

  app.get("/api/session", async (request) => {
    return sessionBody(deps, currentSession(deps, request))
  })

  app.post("/api/faucet/claim", async (request, reply) => {
    if (!rateLimit(deps, request, "claim")) return rateLimited(reply)

    const active = currentSession(deps, request)
    if (!active) return reply.code(401).send({ error: "github_session_required" })

    const parsed = claimRequestSchema.safeParse(request.body)
    if (!parsed.success) return reply.code(400).send({ error: "account_id_required" })

    const accountId = normalizeL2Address(parsed.data.account_id)
    rejectZeroAddress(accountId)
    const result = deps.store.createClaim({
      user: active.user,
      accountId,
      amountEnt: deps.config.amountEnt,
      cooldownSeconds: deps.config.cooldownSeconds,
      enforceCooldown: deps.config.enforceCooldown,
    })

    if (!result.ok && result.code === "cooldown") {
      return reply.code(429).send({ error: "cooldown", retryAt: result.retryAt })
    }
    if (!result.ok) {
      return reply.code(202).send({ claim: claimBody(result.claim), duplicate: true })
    }

    return reply.code(202).send({ claim: claimBody(result.claim), duplicate: false })
  })

  app.get("/api/faucet/status", async (request) => {
    const active = currentSession(deps, request)
    return {
      config: publicConfig(deps.config),
      pendingCount: deps.store.pendingCount(),
      session: sessionBody(deps, active),
      claims: active ? deps.store.sessionClaims(active.user.id).map(claimBody) : [],
    }
  })

  app.get("/api/faucet/batches", async () => {
    return { batches: deps.store.safeBatches() }
  })
}

function registerErrorHandling(app: FastifyInstance) {
  app.setErrorHandler((error, _request, reply) => {
    if (error instanceof AddressError) {
      return reply.code(400).send({ error: error.message })
    }
    if (error instanceof ZodError) {
      return reply.code(400).send({ error: "invalid_request" })
    }
    if (hasStatusCode(error) && error.statusCode === 413) {
      return reply.code(413).send({ error: "request_too_large" })
    }

    return reply.code(500).send({ error: "faucet_internal_error" })
  })
}

function hasStatusCode(error: unknown): error is { statusCode: number } {
  return (
    typeof error === "object" &&
    error !== null &&
    "statusCode" in error &&
    typeof (error as { statusCode?: unknown }).statusCode === "number"
  )
}

function currentSession(deps: FaucetDeps, request: FastifyRequest) {
  return deps.store.getSession(cookieValue(deps, request))
}

function cookieValue(deps: FaucetDeps, request: FastifyRequest) {
  return request.cookies[deps.config.sessionCookieName] ?? null
}

function sessionBody(deps: FaucetDeps, active: ReturnType<FaucetStore["getSession"]>) {
  return {
    authenticated: Boolean(active),
    user: active
      ? { id: active.user.id, login: active.user.login, avatarUrl: active.user.avatarUrl }
      : null,
    config: publicConfig(deps.config),
  }
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
  }
}

function rateLimit(deps: FaucetDeps, request: FastifyRequest, action: string) {
  return deps.store.checkRateLimit(
    `${action}:${request.ip}`,
    deps.config.rateLimitMax,
    deps.config.rateLimitWindowMs,
  )
}

function rateLimited(reply: FastifyReply) {
  return reply.code(429).send({ error: "rate_limited" })
}

function requestOrigin(request: FastifyRequest) {
  const proto = request.headers["x-forwarded-proto"] ?? request.protocol
  const host = request.headers["x-forwarded-host"] ?? request.headers.host ?? "127.0.0.1:3002"
  return `${Array.isArray(proto) ? proto[0] : proto}://${Array.isArray(host) ? host[0] : host}`
}

function requestIsSecure(request: FastifyRequest) {
  return requestOrigin(request).startsWith("https://")
}
