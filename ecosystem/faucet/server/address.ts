export const L2_RAW_ADDRESS_PREFIX = "8:"
export const L2_USER_FRIENDLY_PREFIX = "EX"
export const L2_USER_FRIENDLY_LENGTH = 48
export const L2_ZERO_ACCOUNT_ID = "0".repeat(64)

const L2_USER_FRIENDLY_TAG = 0x11
const L2_USER_FRIENDLY_NETWORK = 0x78

export class AddressError extends Error {
  constructor(message = "invalid_l2_address") {
    super(message)
  }
}

export function normalizeL2Address(value: string) {
  const trimmed = value.trim()

  if (trimmed.startsWith(L2_RAW_ADDRESS_PREFIX)) {
    return normalizeHash32(trimmed.slice(L2_RAW_ADDRESS_PREFIX.length))
  }

  if (trimmed.length === L2_USER_FRIENDLY_LENGTH && trimmed.startsWith(L2_USER_FRIENDLY_PREFIX)) {
    return parseUserFriendlyAddress(trimmed)
  }

  return normalizeHash32(trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed)
}

export function l2RawAddress(accountId: string) {
  return `${L2_RAW_ADDRESS_PREFIX}${normalizeHash32(accountId)}`
}

export function rejectZeroAddress(accountId: string) {
  if (normalizeHash32(accountId) === L2_ZERO_ACCOUNT_ID) {
    throw new AddressError("reserved_zero_address")
  }
}

function normalizeHash32(value: string) {
  if (!/^[0-9a-fA-F]{64}$/u.test(value)) {
    throw new AddressError()
  }

  return value.toLowerCase()
}

function parseUserFriendlyAddress(value: string) {
  const payload = base64UrlDecode(value)
  if (
    payload.length !== 36 ||
    payload[0] !== L2_USER_FRIENDLY_TAG ||
    payload[1] !== L2_USER_FRIENDLY_NETWORK
  ) {
    throw new AddressError()
  }

  const expected = crc16Xmodem(payload.subarray(0, 34))
  const actual = payload.readUInt16BE(34)
  if (expected !== actual) {
    throw new AddressError("invalid_l2_address_checksum")
  }

  return payload.subarray(2, 34).toString("hex")
}

function base64UrlDecode(value: string) {
  if (!/^[A-Za-z0-9_-]+$/u.test(value)) {
    throw new AddressError()
  }

  const base64 = value.replaceAll("-", "+").replaceAll("_", "/")
  return Buffer.from(base64.padEnd(Math.ceil(base64.length / 4) * 4, "="), "base64")
}

function crc16Xmodem(bytes: Buffer) {
  let crc = 0

  for (const byte of bytes) {
    crc ^= byte << 8
    for (let index = 0; index < 8; index += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff
    }
  }

  return crc
}
