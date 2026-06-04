export type Hash32 = string;

export const L2_RAW_ADDRESS_PREFIX = "8:";
export const L2_USER_FRIENDLY_PREFIX = "EX";
export const L2_USER_FRIENDLY_LENGTH = 48;

const L2_USER_FRIENDLY_TAG = 0x11;
const L2_USER_FRIENDLY_NETWORK = 0x78;

export function normalizeHash32(value: string): Hash32 {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return cleaned.toLowerCase();
}

export function l2RawAddress(accountId: string): string {
  return `${L2_RAW_ADDRESS_PREFIX}${normalizeHash32(accountId)}`;
}

export function l2UserFriendlyAddress(accountId: string): string {
  const payload = Buffer.alloc(36);
  payload[0] = L2_USER_FRIENDLY_TAG;
  payload[1] = L2_USER_FRIENDLY_NETWORK;
  Buffer.from(normalizeHash32(accountId), "hex").copy(payload, 2);
  const checksum = crc16Xmodem(payload.subarray(0, 34));
  payload.writeUInt16BE(checksum, 34);
  return base64UrlEncode(payload);
}

export function parseL2Address(value: string): Hash32 {
  if (value.startsWith(L2_RAW_ADDRESS_PREFIX)) {
    return normalizeHash32(value.slice(L2_RAW_ADDRESS_PREFIX.length));
  }
  if (value.length === L2_USER_FRIENDLY_LENGTH && value.startsWith(L2_USER_FRIENDLY_PREFIX)) {
    return parseUserFriendlyAddress(value);
  }
  return normalizeHash32(value);
}

function parseUserFriendlyAddress(value: string): Hash32 {
  const payload = base64UrlDecode(value);
  if (
    payload.length !== 36 ||
    payload[0] !== L2_USER_FRIENDLY_TAG ||
    payload[1] !== L2_USER_FRIENDLY_NETWORK
  ) {
    throw new Error("invalid L2 user-friendly address");
  }
  const expected = crc16Xmodem(payload.subarray(0, 34));
  const actual = payload.readUInt16BE(34);
  if (expected !== actual) {
    throw new Error("invalid L2 user-friendly address checksum");
  }
  return payload.subarray(2, 34).toString("hex");
}

function base64UrlEncode(value: Buffer): string {
  return value
    .toString("base64")
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/u, "");
}

function base64UrlDecode(value: string): Buffer {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) {
    throw new Error("invalid L2 user-friendly address");
  }
  const base64 = value.replaceAll("-", "+").replaceAll("_", "/");
  return Buffer.from(base64.padEnd(Math.ceil(base64.length / 4) * 4, "="), "base64");
}

function crc16Xmodem(bytes: Buffer): number {
  let crc = 0;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let i = 0; i < 8; i += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc;
}
