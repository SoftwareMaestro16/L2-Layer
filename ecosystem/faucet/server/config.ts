export type FaucetConfig = {
  entropisApiUrl: string;
  l2AdminToken: string | null;
  githubClientId: string | null;
  githubClientSecret: string | null;
  amountEnt: number;
  batchIntervalMs: number;
  cooldownSeconds: number;
  enforceCooldown: boolean;
  maxBatchSize: number;
  host: string;
  port: number;
  sessionCookieName: string;
  sessionTtlMs: number;
  rateLimitWindowMs: number;
  rateLimitMax: number;
};

export function loadConfig(env: NodeJS.ProcessEnv = process.env): FaucetConfig {
  return {
    entropisApiUrl: stringEnv(env, "ENTROPIS_API_URL", "http://127.0.0.1:3000"),
    l2AdminToken: optionalStringEnv(env, "L2_ADMIN_TOKEN"),
    githubClientId: optionalStringEnv(env, "GITHUB_CLIENT_ID"),
    githubClientSecret: optionalStringEnv(env, "GITHUB_CLIENT_SECRET"),
    amountEnt: intEnv(env, "FAUCET_AMOUNT_ENT", 100),
    batchIntervalMs: intEnv(env, "FAUCET_BATCH_INTERVAL_MS", 10_000),
    cooldownSeconds: intEnv(env, "FAUCET_COOLDOWN_SECONDS", 7_200),
    enforceCooldown: boolEnv(env, "FAUCET_ENFORCE_COOLDOWN", false),
    maxBatchSize: intEnv(env, "FAUCET_MAX_BATCH_SIZE", 100),
    host: stringEnv(env, "FAUCET_HOST", "127.0.0.1"),
    port: intEnv(env, "FAUCET_PORT", 3003),
    sessionCookieName: stringEnv(env, "FAUCET_SESSION_COOKIE", "entropis_faucet_session"),
    sessionTtlMs: intEnv(env, "FAUCET_SESSION_TTL_MS", 86_400_000),
    rateLimitWindowMs: intEnv(env, "FAUCET_RATE_LIMIT_WINDOW_MS", 60_000),
    rateLimitMax: intEnv(env, "FAUCET_RATE_LIMIT_MAX", 30),
  };
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
  };
}

function stringEnv(env: NodeJS.ProcessEnv, key: string, fallback: string): string {
  const value = env[key]?.trim();
  return value && value.length > 0 ? value : fallback;
}

function optionalStringEnv(env: NodeJS.ProcessEnv, key: string): string | null {
  const value = env[key]?.trim();
  return value && value.length > 0 ? value : null;
}

function intEnv(env: NodeJS.ProcessEnv, key: string, fallback: number): number {
  const raw = env[key]?.trim();
  if (!raw) {
    return fallback;
  }
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${key} must be a positive integer`);
  }
  return value;
}

function boolEnv(env: NodeJS.ProcessEnv, key: string, fallback: boolean): boolean {
  const raw = env[key]?.trim().toLowerCase();
  if (!raw) {
    return fallback;
  }
  if (raw === "true" || raw === "1") {
    return true;
  }
  if (raw === "false" || raw === "0") {
    return false;
  }
  throw new Error(`${key} must be true or false`);
}
