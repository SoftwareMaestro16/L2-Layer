import assert from "node:assert/strict"
import { test } from "node:test"

import { AddressError, normalizeL2Address, rejectZeroAddress } from "./address.js"
import { createFaucetApp } from "./app.js"
import { loadConfig } from "./config.js"
import { GitHubOAuthClient } from "./github.js"
import { EntropisNodeClient } from "./node-client.js"
import { FaucetStore } from "./store.js"
import { FaucetBatchWorker } from "./worker.js"

const ACCOUNT_A = "1".repeat(64)
const ACCOUNT_B = "2".repeat(64)
const USER = { id: 42, login: "octo", avatarUrl: "https://example.test/a.png" }

test("config uses faucet defaults and parses env overrides", () => {
  const defaults = loadConfig({})
  assert.equal(defaults.amountEnt, 100)
  assert.equal(defaults.batchIntervalMs, 10_000)
  assert.equal(defaults.cooldownSeconds, 7_200)
  assert.equal(defaults.enforceCooldown, false)
  assert.equal(defaults.maxBatchSize, 100)
  assert.equal(defaults.port, 3002)

  const config = loadConfig({
    ENTROPIS_API_URL: "http://node.test",
    L2_ADMIN_TOKEN: "admin-token",
    GITHUB_CLIENT_ID: "client",
    GITHUB_CLIENT_SECRET: "secret",
    FAUCET_ENFORCE_COOLDOWN: "true",
  })
  assert.equal(config.entropisApiUrl, "http://node.test")
  assert.equal(config.l2AdminToken, "admin-token")
  assert.equal(config.githubClientId, "client")
  assert.equal(config.githubClientSecret, "secret")
  assert.equal(config.enforceCooldown, true)
})

test("address validation accepts raw forms and rejects zero address", () => {
  assert.equal(normalizeL2Address(`8:${ACCOUNT_A}`), ACCOUNT_A)
  assert.equal(normalizeL2Address(`0x${ACCOUNT_A}`), ACCOUNT_A)
  assert.throws(() => normalizeL2Address("not-an-address"), AddressError)
  assert.throws(() => rejectZeroAddress("0".repeat(64)), /reserved_zero_address/u)
})

test("github oauth URL includes state and PKCE", () => {
  const store = new FaucetStore(() => 1_000)
  const oauth = store.createOAuthState("http://127.0.0.1:3002/api/auth/github/callback")
  const client = new GitHubOAuthClient({ clientId: "client", clientSecret: "secret" })
  const url = new URL(client.authorizationUrl(oauth))

  assert.equal(url.searchParams.get("state"), oauth.state)
  assert.equal(url.searchParams.get("code_challenge"), oauth.codeChallenge)
  assert.equal(url.searchParams.get("code_challenge_method"), "S256")
  assert.equal(url.searchParams.get("scope"), "read:user")
})

test("github callback exchanges code with verifier and does not expose token in user", async () => {
  const bodies: unknown[] = []
  const fetchImpl = async (input: string | URL | Request, init?: RequestInit) => {
    if (String(input).includes("access_token")) {
      bodies.push(JSON.parse(String(init?.body)))
      return jsonResponse({ access_token: "github-token" })
    }
    return jsonResponse({ id: 7, login: "alice", avatar_url: "https://avatar.test/a.png" })
  }
  const client = new GitHubOAuthClient({ clientId: "client", clientSecret: "secret" }, fetchImpl)
  const user = await client.completeCallback({
    code: "code",
    redirectUri: "http://127.0.0.1:3002/api/auth/github/callback",
    codeVerifier: "verifier",
  })

  assert.equal(user.id, 7)
  assert.equal(user.login, "alice")
  assert.deepEqual(Object.keys(user).sort(), ["avatarUrl", "id", "login"])
  assert.equal((bodies[0] as { code_verifier?: string }).code_verifier, "verifier")
})

test("store queues claims and only enforces cooldown when enabled", () => {
  let now = 1_000
  const store = new FaucetStore(() => now)
  const first = store.createClaim({
    user: USER,
    accountId: ACCOUNT_A,
    amountEnt: 100,
    cooldownSeconds: 7_200,
    enforceCooldown: false,
  })
  assert.equal(first.ok, true)
  assert.equal(store.pendingCount(), 1)

  const pendingDuplicate = store.createClaim({
    user: USER,
    accountId: ACCOUNT_A,
    amountEnt: 100,
    cooldownSeconds: 7_200,
    enforceCooldown: false,
  })
  assert.equal(pendingDuplicate.ok, false)
  assert.equal(pendingDuplicate.ok ? "" : pendingDuplicate.code, "already_pending")

  const taken = store.takePending(10)
  store.completeBatch(store.startBatch(taken).batchId, [
    { claimId: taken[0]!.claimId, status: "granted", depositId: "deposit", error: null },
  ])
  now += 1_000
  assert.equal(
    store.createClaim({
      user: USER,
      accountId: ACCOUNT_A,
      amountEnt: 100,
      cooldownSeconds: 7_200,
      enforceCooldown: false,
    }).ok,
    true,
  )
})

test("store enforces github and account cooldown when configured", () => {
  const store = new FaucetStore(() => 1_000)
  const first = store.createClaim({
    user: USER,
    accountId: ACCOUNT_A,
    amountEnt: 100,
    cooldownSeconds: 7_200,
    enforceCooldown: true,
  })
  assert.equal(first.ok, true)

  const sameGithub = store.createClaim({
    user: USER,
    accountId: ACCOUNT_B,
    amountEnt: 100,
    cooldownSeconds: 7_200,
    enforceCooldown: true,
  })
  assert.equal(sameGithub.ok, false)
  assert.equal(sameGithub.ok ? "" : sameGithub.code, "cooldown")
})

test("worker drains queue and records granted result", async () => {
  const store = new FaucetStore(() => 1_000)
  store.createClaim({
    user: USER,
    accountId: ACCOUNT_A,
    amountEnt: 100,
    cooldownSeconds: 7_200,
    enforceCooldown: false,
  })
  const worker = new FaucetBatchWorker(loadConfig({}), store, {
    submitClaims: async (claims) =>
      claims.map((claim) => ({
        claimId: claim.claimId,
        status: "granted",
        depositId: "deposit",
        error: null,
      })),
  } as EntropisNodeClient)

  await worker.drainOnce()
  assert.equal(store.pendingCount(), 0)
  assert.equal(store.sessionClaims(USER.id)[0]?.status, "granted")
  assert.equal(store.safeBatches()[0]?.status, "submitted")
})

test("node client uses batch endpoint and maps missing claim as failure", async () => {
  const client = new EntropisNodeClient(
    loadConfig({ ENTROPIS_API_URL: "http://node.test", L2_ADMIN_TOKEN: "admin-token" }),
    async () =>
      jsonResponse({
        claims: [{ claim_id: "claim-a", faucet: { granted: true, deposit_id: "deposit-a" } }],
      }),
  )
  const results = await client.submitClaims([
    { claimId: "claim-a", accountId: ACCOUNT_A },
    { claimId: "claim-b", accountId: ACCOUNT_B },
  ])

  assert.equal(results[0]?.status, "granted")
  assert.equal(results[1]?.status, "failed")
  assert.equal(results[1]?.error, "node_batch_missing_claim")
})

test("node client falls back to single faucet endpoint on missing batch route", async () => {
  const calls: string[] = []
  const client = new EntropisNodeClient(
    loadConfig({ ENTROPIS_API_URL: "http://node.test", L2_ADMIN_TOKEN: "admin-token" }),
    async (input) => {
      calls.push(String(input))
      if (String(input).endsWith("/batch")) return new Response("", { status: 404 })
      if (calls.length === 2) return jsonResponse({ granted: true, deposit_id: "deposit-a" })
      return new Response(JSON.stringify({ error: "bad" }), { status: 400 })
    },
  )
  const results = await client.submitClaims([
    { claimId: "claim-a", accountId: ACCOUNT_A },
    { claimId: "claim-b", accountId: ACCOUNT_B },
  ])

  assert.equal(results[0]?.status, "granted")
  assert.equal(results[1]?.status, "failed")
  assert.equal(results[1]?.error, "node_claim_rejected")
})

test("fastify app exposes status and gates claims behind github session", async () => {
  const app = await createFaucetApp({
    config: loadConfig({}),
    store: new FaucetStore(() => 1_000),
    githubClient: null,
  })

  const status = await app.inject({ method: "GET", url: "/api/faucet/status" })
  assert.equal(status.statusCode, 200)
  assert.equal(status.json().session.authenticated, false)

  const claim = await app.inject({
    method: "POST",
    url: "/api/faucet/claim",
    payload: { account_id: ACCOUNT_A },
  })
  assert.equal(claim.statusCode, 401)
  assert.equal(claim.json().error, "github_session_required")

  await app.close()
})

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  })
}
