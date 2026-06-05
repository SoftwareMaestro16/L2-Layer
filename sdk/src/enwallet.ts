import { beginCell, Cell, Dictionary, storeStateInit } from "@ton/core";
import { mnemonicWordList } from "@ton/crypto";
import nacl from "tweetnacl";
import { type Hash32 } from "./address.js";
import { deriveAccountId, hashDomain } from "./consensus.js";
import {
  contractCellHash,
  signDeployContractTransaction,
  type DeployContractTransactionParams,
  type SignedL2Transaction,
} from "./contracts.js";
import {
  EnWalletV5,
  ExternalSignedRequest,
  InternalSignedRequest,
  Storage as EnWalletStorage,
} from "./generated/EnWalletV5.gen.js";

type UIntLike = bigint | number | string;

export const ENWALLET_V5R1_INTERFACE = "org.ton.wallet.v5.r1";
export const ENWALLET_V5R1_LABEL = "Wallet Signed External V5 R1";
export const ENWALLET_V5R1_TESTNET_WALLET_ID = 0x7ffffffdn;
const ENWALLET_ED25519_SEED_DOMAIN = "entropis.enwallet.ed25519.seed.v1";
const ENWALLET_MNEMONIC_WORD_COUNT = 24;
const ENWALLET_MNEMONIC_ENTROPY_BYTES = 32;
const BIP39_PBKDF2_ITERATIONS = 2048;
const BIP39_SEED_BITS = 512;
const BIP39_WORD_BITS = 11;
const BIP39_WORDS = mnemonicWordList as readonly string[];
const BIP39_WORD_INDEX = new Map(BIP39_WORDS.map((word, index) => [word, index]));

export interface EnWalletV5InitialState {
  interface: typeof ENWALLET_V5R1_INTERFACE;
  interface_label: typeof ENWALLET_V5R1_LABEL;
  owner_account_id: Hash32;
  wallet_account_id: Hash32;
  wallet_id: string;
  public_key: string;
  code_hash: Hash32;
  data_hash: Hash32;
  storage_root: Hash32;
  code_boc_base64: string;
  data_boc_base64: string;
}

export interface EnWalletV5InitParams {
  publicKey: Uint8Array | string;
  walletId?: UIntLike;
}

export interface EnWalletV5DeployParams
  extends Omit<DeployContractTransactionParams, "contract" | "codeBocBase64" | "dataBocBase64"> {
  walletId?: UIntLike;
  keyPair: nacl.SignKeyPair;
}

export async function createEnWalletMnemonic(): Promise<string[]> {
  return mnemonicFromEntropy(randomBytes(ENWALLET_MNEMONIC_ENTROPY_BYTES));
}

export async function validateEnWalletMnemonic(words: string[]): Promise<boolean> {
  return validateBip39Mnemonic(words);
}

export async function enwalletKeyPairFromMnemonic(words: string[]): Promise<nacl.SignKeyPair> {
  if (!(await validateEnWalletMnemonic(words))) {
    throw new Error("invalid EnWallet mnemonic");
  }
  const mnemonicSeed = await bip39Seed(normalizeMnemonicWords(words));
  const signingSeed = await sha256Bytes(
    concatBytes(utf8Bytes(ENWALLET_ED25519_SEED_DOMAIN), mnemonicSeed),
  );
  return nacl.sign.keyPair.fromSeed(signingSeed);
}

export function enwalletV5CodeBocBase64(): string {
  return EnWalletV5.CodeCell.toBoc().toString("base64");
}

export function enwalletV5CodeHash(): Hash32 {
  return EnWalletV5.CodeCell.hash().toString("hex");
}

export function enwalletV5DataCell(params: EnWalletV5InitParams): Cell {
  return EnWalletStorage.toCell(
    EnWalletStorage.create({
      isSignatureAllowed: true,
      seqno: 0n,
      subwalletId: toUint(params.walletId ?? ENWALLET_V5R1_TESTNET_WALLET_ID, "walletId", 32),
      publicKey: publicKeyToUint256(params.publicKey),
      extensions: Dictionary.empty(Dictionary.Keys.BigUint(256), Dictionary.Values.Bool()),
    }),
  );
}

export function enwalletV5DataBocBase64(params: EnWalletV5InitParams): string {
  return enwalletV5DataCell(params).toBoc().toString("base64");
}

export function enwalletV5StateInitCell(params: EnWalletV5InitParams): Cell {
  return beginCell()
    .store(
      storeStateInit({
        code: EnWalletV5.CodeCell,
        data: enwalletV5DataCell(params),
        libraries: null,
        special: null,
        splitDepth: null,
      }),
    )
    .endCell();
}

export function enwalletV5AccountId(params: EnWalletV5InitParams): Hash32 {
  return enwalletV5StateInitCell(params).hash().toString("hex");
}

export function enwalletV5InitialState(params: EnWalletV5InitParams): EnWalletV5InitialState {
  const publicKey = normalizePublicKey(params.publicKey);
  const data_boc_base64 = enwalletV5DataBocBase64({
    publicKey,
    walletId: params.walletId,
  });
  const data_hash = contractCellHash(data_boc_base64);
  return {
    interface: ENWALLET_V5R1_INTERFACE,
    interface_label: ENWALLET_V5R1_LABEL,
    owner_account_id: deriveAccountId(publicKey),
    wallet_account_id: enwalletV5AccountId({
      publicKey,
      walletId: params.walletId,
    }),
    wallet_id: toUint(params.walletId ?? ENWALLET_V5R1_TESTNET_WALLET_ID, "walletId", 32).toString(),
    public_key: Buffer.from(publicKey).toString("hex"),
    code_hash: enwalletV5CodeHash(),
    data_hash,
    storage_root: data_hash,
    code_boc_base64: enwalletV5CodeBocBase64(),
    data_boc_base64,
  };
}

export function signEnWalletV5InitTransaction(
  params: EnWalletV5DeployParams,
): SignedL2Transaction {
  const initial = enwalletV5InitialState({
    publicKey: params.keyPair.publicKey,
    walletId: params.walletId,
  });
  return signDeployContractTransaction({
    ...params,
    contract: initial.wallet_account_id,
    codeBocBase64: initial.code_boc_base64,
    dataBocBase64: initial.data_boc_base64,
  });
}

export function enwalletV5SignedInternalBodyBase64(params: {
  keyPair: nacl.SignKeyPair;
  walletId?: UIntLike;
  validUntil: UIntLike;
  seqno: UIntLike;
}): string {
  return signWalletRequest(
    InternalSignedRequest.toCell(
      InternalSignedRequest.create({
        walletId: toUint(params.walletId ?? ENWALLET_V5R1_TESTNET_WALLET_ID, "walletId", 32),
        validUntil: toUint(params.validUntil, "validUntil", 32),
        seqno: toUint(params.seqno, "seqno", 32),
        outActions: null,
        hasExtraActions: false,
        extraActions: Cell.EMPTY.beginParse(),
      }),
    ),
    params.keyPair,
  );
}

export function enwalletV5SignedExternalBodyBase64(params: {
  keyPair: nacl.SignKeyPair;
  walletId?: UIntLike;
  validUntil: UIntLike;
  seqno: UIntLike;
}): string {
  return signWalletRequest(
    ExternalSignedRequest.toCell(
      ExternalSignedRequest.create({
        walletId: toUint(params.walletId ?? ENWALLET_V5R1_TESTNET_WALLET_ID, "walletId", 32),
        validUntil: toUint(params.validUntil, "validUntil", 32),
        seqno: toUint(params.seqno, "seqno", 32),
        outActions: null,
        hasExtraActions: false,
        extraActions: Cell.EMPTY.beginParse(),
      }),
    ),
    params.keyPair,
  );
}

export function enwalletV5ContractSalt(publicKey: Uint8Array | string, walletId?: UIntLike): Hash32 {
  const walletIdBytes = Buffer.alloc(4);
  walletIdBytes.writeUInt32BE(Number(toUint(walletId ?? ENWALLET_V5R1_TESTNET_WALLET_ID, "walletId", 32)));
  return hashDomain("entropis.enwallet.v5r1.v1", [normalizePublicKey(publicKey), walletIdBytes]);
}

function signWalletRequest(unsigned: Cell, keyPair: nacl.SignKeyPair): string {
  const signature = nacl.sign.detached(unsigned.hash(), keyPair.secretKey);
  return beginCell()
    .storeSlice(unsigned.beginParse())
    .storeBuffer(Buffer.from(signature), 64)
    .endCell()
    .toBoc()
    .toString("base64");
}

function normalizePublicKey(value: Uint8Array | string): Uint8Array {
  if (typeof value !== "string") {
    if (value.length !== 32) {
      throw new Error("publicKey must be 32 bytes");
    }
    return value;
  }
  const cleaned = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
    throw new Error("publicKey must be 32-byte hex");
  }
  return Buffer.from(cleaned, "hex");
}

function publicKeyToUint256(value: Uint8Array | string): bigint {
  return BigInt(`0x${Buffer.from(normalizePublicKey(value)).toString("hex")}`);
}

function toUint(value: UIntLike, field: string, bits: number): bigint {
  const parsed = parseUint(value, field);
  if (parsed >= (1n << BigInt(bits))) {
    throw new Error(`${field} exceeds uint${bits}`);
  }
  return parsed;
}

function normalizeMnemonicWords(words: string[]): string[] {
  return words
    .map((word) => word.trim().toLowerCase())
    .filter(Boolean);
}

async function validateBip39Mnemonic(words: string[]): Promise<boolean> {
  const normalized = normalizeMnemonicWords(words);
  if (normalized.length !== ENWALLET_MNEMONIC_WORD_COUNT) {
    return false;
  }
  const indices: number[] = [];
  for (const word of normalized) {
    const index = BIP39_WORD_INDEX.get(word);
    if (index === undefined) {
      return false;
    }
    indices.push(index);
  }

  const bits = indices.map((index) => index.toString(2).padStart(BIP39_WORD_BITS, "0")).join("");
  const entropyBitsLength = Math.floor((bits.length * 32) / 33);
  const checksumBitsLength = bits.length - entropyBitsLength;
  if (entropyBitsLength !== ENWALLET_MNEMONIC_ENTROPY_BYTES * 8) {
    return false;
  }

  const entropyBits = bits.slice(0, entropyBitsLength);
  const checksumBits = bits.slice(entropyBitsLength);
  const entropy = bitsToBytes(entropyBits);
  const expectedChecksum = bytesToBits(await sha256Bytes(entropy)).slice(0, checksumBitsLength);
  return checksumBits === expectedChecksum;
}

async function bip39Seed(words: string[]): Promise<Uint8Array> {
  const phraseText = normalizeMnemonicWords(words).join(" ").normalize("NFKD");
  const key = await webCrypto().subtle.importKey(
    "raw",
    toArrayBuffer(utf8Bytes(phraseText)),
    "PBKDF2",
    false,
    ["deriveBits"],
  );
  const bits = await webCrypto().subtle.deriveBits(
    {
      name: "PBKDF2",
      hash: "SHA-512",
      salt: toArrayBuffer(utf8Bytes("mnemonic")),
      iterations: BIP39_PBKDF2_ITERATIONS,
    },
    key,
    BIP39_SEED_BITS,
  );
  return new Uint8Array(bits);
}

async function mnemonicFromEntropy(entropy: Uint8Array): Promise<string[]> {
  if (entropy.length !== ENWALLET_MNEMONIC_ENTROPY_BYTES) {
    throw new Error("EnWallet mnemonic entropy must be 32 bytes");
  }
  const checksumLength = (entropy.length * 8) / 32;
  const bits = `${bytesToBits(entropy)}${bytesToBits(await sha256Bytes(entropy)).slice(0, checksumLength)}`;
  const words: string[] = [];
  for (let offset = 0; offset < bits.length; offset += BIP39_WORD_BITS) {
    const index = Number.parseInt(bits.slice(offset, offset + BIP39_WORD_BITS), 2);
    words.push(BIP39_WORDS[index]);
  }
  return words;
}

function bitsToBytes(bits: string): Uint8Array {
  if (bits.length % 8 !== 0) {
    throw new Error("bit string length must be byte-aligned");
  }
  const bytes = new Uint8Array(bits.length / 8);
  for (let offset = 0; offset < bits.length; offset += 8) {
    bytes[offset / 8] = Number.parseInt(bits.slice(offset, offset + 8), 2);
  }
  return bytes;
}

function bytesToBits(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(2).padStart(8, "0")).join("");
}

async function sha256Bytes(bytes: Uint8Array): Promise<Uint8Array> {
  return new Uint8Array(await webCrypto().subtle.digest("SHA-256", toArrayBuffer(bytes)));
}

function randomBytes(length: number): Uint8Array {
  const bytes = new Uint8Array(length);
  webCrypto().getRandomValues(bytes);
  return bytes;
}

function concatBytes(...chunks: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.length, 0));
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function utf8Bytes(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const out = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(out).set(bytes);
  return out;
}

function webCrypto(): Crypto {
  if (!globalThis.crypto?.subtle) {
    throw new Error("Web Crypto API is required for EnWallet mnemonic derivation");
  }
  return globalThis.crypto;
}

function parseUint(value: UIntLike, field: string): bigint {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new Error(`${field} must be non-negative`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${field} must be a non-negative safe integer`);
    }
    return BigInt(value);
  }
  if (!/^[0-9]+$/.test(value)) {
    throw new Error(`${field} must be an unsigned decimal integer`);
  }
  return BigInt(value);
}
