import { generateMnemonic, mnemonicToSeedSync, validateMnemonic } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, concatBytes, hexToBytes, utf8ToBytes } from "@noble/hashes/utils.js";
import nacl from "tweetnacl";
import {
  l2RawAddress,
  l2UserFriendlyAddress,
  normalizeHash32,
  parseL2Address
} from "@/lib/l2-address";

export const ENTROPIS_CHAIN_ID = "entropis-testnet";
export const ENTROPIS_DECIMALS = 9;
export const ENTROPIS_GAS_LIMIT = 1000;
export const ENTROPIS_MAX_GAS_PRICE = 1;
export const ENTROPIS_TX_TTL_BLOCKS = 1000;
export const L2_TX_VERSION_V2 = 2;
export const L2_TRANSACTION_KIND_VERSION_V1 = 1;
export const L2_TX_DOMAIN_SEPARATOR = "entropis.l2.tx.v2";
export {
  L2_RAW_ADDRESS_PREFIX,
  L2_USER_FRIENDLY_LENGTH,
  L2_USER_FRIENDLY_PREFIX,
  l2RawAddress,
  l2UserFriendlyAddress,
  parseL2Address,
  shortAddress
} from "@/lib/l2-address";

const CONSENSUS_ENCODING_VERSION = 1;
const KIND_TRANSFER = 0x02;
const MAGIC = utf8ToBytes("EL2C");
const TYPE_UNSIGNED_TX = 0x01;
const ENWALLET_KEY_DOMAIN = "entropis.enwallet.ed25519.seed.v1";

export type EnWalletKeyPair = {
  publicKey: Uint8Array;
  secretKey: Uint8Array;
};

export type EnWalletIdentity = {
  recoveryWords: string;
  keyPair: EnWalletKeyPair;
  accountId: string;
  rawAddress: string;
  friendlyAddress: string;
  publicKeyHex: string;
};

export type SignedL2Transaction = {
  tx_version: number;
  domain_separator: string;
  chain_id: string;
  from: string | null;
  nonce: number;
  valid_until_block: number;
  gas_limit: number;
  max_gas_price: string;
  fee_asset_id: number;
  memo_hash: string | null;
  transaction_kind_version: number;
  kind: {
    Transfer: {
      to: string;
      asset_id: number;
      amount: string;
    };
  };
  public_key: string | null;
  signature: string | null;
};

export function createMnemonic24(): string {
  return generateMnemonic(wordlist, 256);
}

export function identityFromMnemonic(mnemonic: string): EnWalletIdentity {
  const normalized = normalizeMnemonic(mnemonic);
  if (!validateMnemonic(normalized, wordlist)) {
    throw new Error("Enter a valid 24-word BIP39 seed phrase.");
  }
  if (normalized.split(" ").length !== 24) {
    throw new Error("EnWallet currently requires a 24-word seed phrase.");
  }

  const seed = mnemonicToSeedSync(normalized);
  const keySeed = sha256(concatBytes(utf8ToBytes(ENWALLET_KEY_DOMAIN), seed));
  const keyPair = nacl.sign.keyPair.fromSeed(keySeed);
  const accountId = deriveAccountId(keyPair.publicKey);

  return {
    recoveryWords: normalized,
    keyPair,
    accountId,
    rawAddress: l2RawAddress(accountId),
    friendlyAddress: l2UserFriendlyAddress(accountId),
    publicKeyHex: bytesToHex(keyPair.publicKey)
  };
}

export function normalizeMnemonic(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, " ");
}

export function formatBaseUnits(value: string | number | bigint, decimals = ENTROPIS_DECIMALS): number {
  const parsed = typeof value === "bigint" ? value : BigInt(value);
  const scale = 10n ** BigInt(decimals);
  const whole = parsed / scale;
  const fraction = parsed % scale;
  return Number(whole) + Number(fraction) / Number(scale);
}

export function parseTokenAmount(value: string, decimals = ENTROPIS_DECIMALS): string {
  const normalized = value.trim();
  if (!/^\d+(\.\d+)?$/.test(normalized)) {
    throw new Error("Amount must be a positive decimal number.");
  }
  const [whole, fraction = ""] = normalized.split(".");
  if (fraction.length > decimals) {
    throw new Error(`Amount supports up to ${decimals} decimals.`);
  }
  const padded = fraction.padEnd(decimals, "0");
  const baseUnits = BigInt(whole) * 10n ** BigInt(decimals) + BigInt(padded || "0");
  if (baseUnits <= 0n) {
    throw new Error("Amount must be greater than zero.");
  }
  return baseUnits.toString();
}

export function signTransferTransaction(params: {
  recoveryWords: string;
  from: string;
  nonce: number;
  to: string;
  amount: string;
  assetId?: number;
  chainId?: string;
  gasLimit?: number;
  maxGasPrice?: number;
  validUntilBlock?: number;
  feeAssetId?: number;
  memoHash?: string | null;
}): SignedL2Transaction {
  const identity = identityFromMnemonic(params.recoveryWords);
  const unsigned: SignedL2Transaction = {
    tx_version: L2_TX_VERSION_V2,
    domain_separator: L2_TX_DOMAIN_SEPARATOR,
    chain_id: params.chainId ?? ENTROPIS_CHAIN_ID,
    from: parseL2Address(params.from),
    nonce: params.nonce,
    valid_until_block: params.validUntilBlock ?? Number.MAX_SAFE_INTEGER,
    gas_limit: params.gasLimit ?? ENTROPIS_GAS_LIMIT,
    max_gas_price: String(params.maxGasPrice ?? ENTROPIS_MAX_GAS_PRICE),
    fee_asset_id: params.feeAssetId ?? 0,
    memo_hash: params.memoHash ? normalizeHash32(params.memoHash) : null,
    transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
    kind: {
      Transfer: {
        to: parseL2Address(params.to),
        asset_id: params.assetId ?? 0,
        amount: params.amount
      }
    },
    public_key: bytesToHex(identity.keyPair.publicKey),
    signature: null
  };
  const signature = nacl.sign.detached(encodeUnsignedTransaction(unsigned), identity.keyPair.secretKey);
  return {
    ...unsigned,
    signature: bytesToHex(signature)
  };
}

export function txHash(tx: SignedL2Transaction): string {
  return hashDomain("l2.tx.v2", [encodeUnsignedTransaction(tx)]);
}

function deriveAccountId(publicKey: Uint8Array): string {
  if (publicKey.length !== 32) {
    throw new Error("ed25519 public key must be 32 bytes");
  }
  return hashDomain("l2.account.ed25519.v1", [publicKey]);
}

function encodeUnsignedTransaction(tx: SignedL2Transaction): Uint8Array {
  const out = new ConsensusWriter(TYPE_UNSIGNED_TX);
  out.u16(tx.tx_version);
  out.string(tx.domain_separator);
  out.string(tx.chain_id);
  out.optionalHash(tx.from);
  out.u64(tx.nonce);
  out.u64(tx.valid_until_block);
  out.u64(tx.gas_limit);
  out.u128(tx.max_gas_price);
  out.u32(tx.fee_asset_id);
  out.optionalHash(tx.memo_hash);
  out.u16(tx.transaction_kind_version);
  out.u8(KIND_TRANSFER);
  out.hash(tx.kind.Transfer.to);
  out.u32(tx.kind.Transfer.asset_id);
  out.u128(tx.kind.Transfer.amount);
  return out.bytes();
}

function hashDomain(domain: string, parts: Uint8Array[]): string {
  const chunks: Uint8Array[] = [];
  const domainBytes = utf8ToBytes(domain);
  chunks.push(u64be(BigInt(domainBytes.length)), domainBytes);
  for (const part of parts) {
    chunks.push(u64be(BigInt(part.length)), part);
  }
  return bytesToHex(sha256(concatBytes(...chunks)));
}

class ConsensusWriter {
  private readonly chunks: Uint8Array[] = [];

  constructor(typeTag: number) {
    this.raw(MAGIC);
    this.u8(CONSENSUS_ENCODING_VERSION);
    this.u8(typeTag);
  }

  raw(value: Uint8Array) {
    this.chunks.push(value);
  }

  u8(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xff) {
      throw new Error("expected uint8");
    }
    this.chunks.push(Uint8Array.of(value));
  }

  u16(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
      throw new Error("expected uint16");
    }
    const out = new Uint8Array(2);
    new DataView(out.buffer).setUint16(0, value, false);
    this.chunks.push(out);
  }

  u32(value: number) {
    if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
      throw new Error("expected uint32");
    }
    const out = new Uint8Array(4);
    new DataView(out.buffer).setUint32(0, value, false);
    this.chunks.push(out);
  }

  u64(value: number | bigint) {
    const bigint = toUint(value, 64);
    this.chunks.push(u64be(bigint));
  }

  u128(value: string | number | bigint) {
    const bigint = toUint(value, 128);
    const out = new Uint8Array(16);
    const view = new DataView(out.buffer);
    view.setBigUint64(0, bigint >> 64n, false);
    view.setBigUint64(8, bigint & 0xffffffffffffffffn, false);
    this.chunks.push(out);
  }

  hash(value: string) {
    this.raw(hexToBytes(normalizeHash32(value)));
  }

  optionalHash(value: string | null) {
    if (value === null) {
      this.u8(0);
      return;
    }
    this.u8(1);
    this.hash(value);
  }

  string(value: string) {
    const bytes = utf8ToBytes(value);
    this.u32(bytes.length);
    this.raw(bytes);
  }

  bytes() {
    return concatBytes(...this.chunks);
  }
}

function toUint(value: string | number | bigint, bits: number): bigint {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(`expected safe uint${bits} number`);
  }
  const bigint = typeof value === "bigint" ? value : BigInt(value);
  if (bigint < 0n || bigint >= 1n << BigInt(bits)) {
    throw new Error(`expected uint${bits}`);
  }
  return bigint;
}

function u64be(value: bigint): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, value, false);
  return out;
}
