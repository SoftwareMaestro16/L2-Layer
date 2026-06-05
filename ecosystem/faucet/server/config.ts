import type { FaucetConfig } from "./types.js"
import { z } from "zod"

const positiveIntegerString = z
  .string()
  .trim()
  .regex(/^[1-9]\d*$/u)
  .refine((value) => Number.isSafeInteger(Number(value)))

const envSchema = z.object({
  ENTROPIS_API_URL: z.string().trim().min(1).optional(),
  L2_ADMIN_TOKEN: z.string().trim().min(1).optional(),
  GITHUB_CLIENT_ID: z.string().trim().min(1).optional(),
  GITHUB_CLIENT_SECRET: z.string().trim().min(1).optional(),
  FAUCET_AMOUNT_ENT: positiveIntegerString.optional(),
  FAUCET_BATCH_INTERVAL_MS: positiveIntegerString.optional(),
  FAUCET_COOLDOWN_SECONDS: positiveIntegerString.optional(),
  FAUCET_ENFORCE_COOLDOWN: z.enum(["true", "false", "1", "0"]).optional(),
  FAUCET_MAX_BATCH_SIZE: positiveIntegerString.optional(),
  FAUCET_HOST: z.string().trim().min(1).optional(),
  FAUCET_PORT: positiveIntegerString.optional(),
  FAUCET_SESSION_COOKIE: z.string().trim().min(1).optional(),
  FAUCET_SESSION_TTL_MS: positiveIntegerString.optional(),
  FAUCET_RATE_LIMIT_WINDOW_MS: positiveIntegerString.optional(),
  FAUCET_RATE_LIMIT_MAX: positiveIntegerString.optional(),
})

export function loadConfig(env: NodeJS.ProcessEnv = process.env): FaucetConfig {
  const parsed = envSchema.parse(env)

  return {
    entropisApiUrl: parsed.ENTROPIS_API_URL ?? "http://127.0.0.1:3000",
    l2AdminToken: parsed.L2_ADMIN_TOKEN ?? null,
    githubClientId: parsed.GITHUB_CLIENT_ID ?? null,
    githubClientSecret: parsed.GITHUB_CLIENT_SECRET ?? null,
    amountEnt: intValue(parsed.FAUCET_AMOUNT_ENT, 100),
    batchIntervalMs: intValue(parsed.FAUCET_BATCH_INTERVAL_MS, 10_000),
    cooldownSeconds: intValue(parsed.FAUCET_COOLDOWN_SECONDS, 7_200),
    enforceCooldown: boolValue(parsed.FAUCET_ENFORCE_COOLDOWN, false),
    maxBatchSize: intValue(parsed.FAUCET_MAX_BATCH_SIZE, 100),
    host: parsed.FAUCET_HOST ?? "127.0.0.1",
    port: intValue(parsed.FAUCET_PORT, 3002),
    sessionCookieName: parsed.FAUCET_SESSION_COOKIE ?? "entropis_faucet_session",
    sessionTtlMs: intValue(parsed.FAUCET_SESSION_TTL_MS, 86_400_000),
    rateLimitWindowMs: intValue(parsed.FAUCET_RATE_LIMIT_WINDOW_MS, 60_000),
    rateLimitMax: intValue(parsed.FAUCET_RATE_LIMIT_MAX, 30),
  }
}

export function publicConfig(config: FaucetConfig) {
  return {
    amountEnt: config.amountEnt,
    batchIntervalMs: config.batchIntervalMs,
    cooldownSeconds: config.cooldownSeconds,
    enforceCooldown: config.enforceCooldown,
    maxBatchSize: config.maxBatchSize,
    githubConfigured: Boolean(config.githubClientId && config.githubClientSecret),
    nodeConfigured: Boolean(config.l2AdminToken),
  }
}

function intValue(raw: string | undefined, fallback: number) {
  return raw ? Number(raw) : fallback
}

function boolValue(raw: "true" | "false" | "1" | "0" | undefined, fallback: boolean) {
  if (!raw) return fallback
  if (raw === "true" || raw === "1") return true
  return false
}
