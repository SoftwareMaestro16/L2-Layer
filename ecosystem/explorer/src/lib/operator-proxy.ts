import { createHash, timingSafeEqual } from "node:crypto";
import { cookies } from "next/headers";

const COOKIE_NAME = "enwatcher_operator";
const COOKIE_MAX_AGE = 60 * 60 * 8;

export const operatorResources = {
  readiness: "/readyz",
  metrics: "/v1/operator/metrics",
  failures: "/v1/operator/failures",
  relayer: "/v1/operator/batch-relayer",
  finalizer: "/v1/operator/batch-finalizer",
} as const;

export type OperatorResource = keyof typeof operatorResources;

export function normalizeApiBase(apiBase?: string): string {
  return (apiBase ?? process.env.ENTROPIS_API_URL ?? "http://127.0.0.1:8080").replace(/\/+$/u, "");
}

export async function createOperatorSession(password: string) {
  const expected = operatorPassword();
  if (!expected) {
    return { ok: false as const, status: 503, error: "operator auth is not configured" };
  }
  if (!constantTimeEqual(hashSecret(password), hashSecret(expected))) {
    return { ok: false as const, status: 401, error: "invalid operator password" };
  }
  const cookieStore = await cookies();
  cookieStore.set(COOKIE_NAME, hashSecret(expected), {
    httpOnly: true,
    sameSite: "strict",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge: COOKIE_MAX_AGE,
  });
  return { ok: true as const };
}

export async function clearOperatorSession() {
  const cookieStore = await cookies();
  cookieStore.delete(COOKIE_NAME);
}

export async function assertOperatorSession() {
  const expected = operatorPassword();
  if (!expected) {
    return { ok: false as const, status: 503, error: "operator auth is not configured" };
  }
  const cookieStore = await cookies();
  const actual = cookieStore.get(COOKIE_NAME)?.value;
  if (!actual || !constantTimeEqual(actual, hashSecret(expected))) {
    return { ok: false as const, status: 401, error: "operator login required" };
  }
  return { ok: true as const };
}

export async function fetchOperatorResource(resource: OperatorResource) {
  const auth = await assertOperatorSession();
  if (!auth.ok) {
    return auth;
  }
  const adminToken = process.env.L2_ADMIN_TOKEN ?? process.env.ENTROPIS_ADMIN_TOKEN;
  if (!adminToken) {
    return { ok: false as const, status: 503, error: "node admin token is not configured" };
  }
  const response = await fetch(`${normalizeApiBase()}${operatorResources[resource]}`, {
    headers: {
      accept: "application/json",
      authorization: `Bearer ${adminToken}`,
    },
    cache: "no-store",
  });
  const text = await response.text();
  const body = text ? safeJson(text) : null;
  if (!response.ok) {
    return {
      ok: false as const,
      status: response.status,
      error: safeError(body, text),
    };
  }
  return { ok: true as const, body };
}

function operatorPassword(): string | undefined {
  return process.env.ENWATCHER_OPERATOR_PASSWORD;
}

function hashSecret(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function constantTimeEqual(left: string, right: string): boolean {
  const leftBytes = Buffer.from(left);
  const rightBytes = Buffer.from(right);
  return leftBytes.length === rightBytes.length && timingSafeEqual(leftBytes, rightBytes);
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function safeError(body: unknown, fallback: string): string {
  if (body && typeof body === "object" && "error" in body && typeof body.error === "string") {
    return body.error;
  }
  return fallback || "operator request failed";
}
