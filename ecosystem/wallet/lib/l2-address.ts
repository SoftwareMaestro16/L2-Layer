import { bytesToHex, hexToBytes } from "@noble/hashes/utils.js";

export const L2_RAW_ADDRESS_PREFIX = "8:";
export const L2_USER_FRIENDLY_PREFIX = "EX";
export const L2_USER_FRIENDLY_LENGTH = 48;

const L2_USER_FRIENDLY_TAG = 0x11;
const L2_USER_FRIENDLY_NETWORK = 0x78;

export function l2RawAddress(accountId: string): string {
  return `${L2_RAW_ADDRESS_PREFIX}${normalizeHash32(accountId)}`;
}

export function l2UserFriendlyAddress(accountId: string): string {
  const payload = new Uint8Array(36);
  payload[0] = L2_USER_FRIENDLY_TAG;
  payload[1] = L2_USER_FRIENDLY_NETWORK;
  payload.set(hexToBytes(normalizeHash32(accountId)), 2);
  const checksum = crc16Xmodem(payload.subarray(0, 34));
  payload[34] = checksum >> 8;
  payload[35] = checksum & 0xff;
  return base64UrlEncode(payload);
}

export function parseL2Address(value: string): string {
  if (value.startsWith(L2_RAW_ADDRESS_PREFIX)) {
    return normalizeHash32(value.slice(L2_RAW_ADDRESS_PREFIX.length));
  }
  if (value.length === L2_USER_FRIENDLY_LENGTH && value.startsWith(L2_USER_FRIENDLY_PREFIX)) {
    return parseUserFriendlyAddress(value);
  }
  return normalizeHash32(value);
}

export function shortAddress(address: string): string {
  if (address.length <= 18) {
    return address;
  }
  return `${address.slice(0, 10)}...${address.slice(-6)}`;
}

export function normalizeHash32(value: string): string {
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("expected 32-byte hex string");
  }
  return cleaned.toLowerCase();
}

function parseUserFriendlyAddress(value: string): string {
  const payload = base64UrlDecode(value);
  if (
    payload.length !== 36 ||
    payload[0] !== L2_USER_FRIENDLY_TAG ||
    payload[1] !== L2_USER_FRIENDLY_NETWORK
  ) {
    throw new Error("invalid L2 user-friendly address");
  }
  const expected = crc16Xmodem(payload.subarray(0, 34));
  const actual = (payload[34] << 8) | payload[35];
  if (expected !== actual) {
    throw new Error("invalid L2 user-friendly address checksum");
  }
  return bytesToHex(payload.subarray(2, 34));
}

function base64UrlEncode(value: Uint8Array): string {
  const raw = Array.from(value, (byte) => String.fromCharCode(byte)).join("");
  return btoa(raw).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}

function base64UrlDecode(value: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) {
    throw new Error("invalid L2 user-friendly address");
  }
  const base64 = value.replaceAll("-", "+").replaceAll("_", "/");
  const raw = atob(base64.padEnd(Math.ceil(base64.length / 4) * 4, "="));
  return Uint8Array.from(raw, (char) => char.charCodeAt(0));
}

function crc16Xmodem(bytes: Uint8Array): number {
  let crc = 0;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let i = 0; i < 8; i += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc;
}
