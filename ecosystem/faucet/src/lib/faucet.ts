import { z } from "zod"

export const FAUCET_AMOUNT = 100
export const COOLDOWN_SECONDS = 60

export const addressSchema = z
  .string()
  .trim()
  .min(12, "Enter a longer Entropis L2 account id.")
  .max(96, "Account id is too long.")
  .regex(/^[A-Za-z0-9:_-]+$/, "Use letters, numbers, colon, dash, or underscore.")

export type FaucetClaim = {
  address: string
  amount: number
  balance: number
  blockHeight: number
  txHash: string
  claimedAt: number
}

export type FaucetResult =
  | { ok: true; claim: FaucetClaim; duplicate: boolean }
  | { ok: false; code: "cooldown"; retryAt: number; claim: FaucetClaim }

const claims = new Map<string, FaucetClaim>()

export function claimMockEnt(address: string): FaucetResult {
  const normalized = address.trim()
  const previous = claims.get(normalized)
  const now = Date.now()

  if (previous) {
    const retryAt = previous.claimedAt + COOLDOWN_SECONDS * 1000

    if (now < retryAt) {
      return { ok: false, code: "cooldown", retryAt, claim: previous }
    }

    const updated = buildClaim(normalized, previous.balance + FAUCET_AMOUNT, now)
    claims.set(normalized, updated)

    return { ok: true, claim: updated, duplicate: false }
  }

  const claim = buildClaim(normalized, startingBalance(normalized) + FAUCET_AMOUNT, now)
  claims.set(normalized, claim)

  return { ok: true, claim, duplicate: false }
}

export function getExistingClaim(address: string) {
  return claims.get(address.trim())
}

function buildClaim(address: string, balance: number, claimedAt: number): FaucetClaim {
  return {
    address,
    amount: FAUCET_AMOUNT,
    balance,
    blockHeight: 420000 + boundedHash(address, 9000),
    txHash: `0x${randomHex(32)}`,
    claimedAt,
  }
}

function startingBalance(address: string) {
  return 20 + boundedHash(address, 40)
}

function boundedHash(input: string, modulo: number) {
  let hash = 0

  for (const char of input) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0
  }

  return hash % modulo
}

function randomHex(bytes: number) {
  const values = new Uint8Array(bytes)
  crypto.getRandomValues(values)

  return Array.from(values, (value) => value.toString(16).padStart(2, "0")).join("")
}
