import { beginCell, Cell, Dictionary, storeStateInit } from "@ton/core";
import {
  mnemonicNew,
  mnemonicToPrivateKey,
  mnemonicValidate,
  type KeyPair,
} from "@ton/crypto";
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
  return mnemonicNew(24);
}

export async function validateEnWalletMnemonic(words: string[]): Promise<boolean> {
  return mnemonicValidate(words);
}

export async function enwalletKeyPairFromMnemonic(words: string[]): Promise<nacl.SignKeyPair> {
  const keyPair = await mnemonicToPrivateKey(words);
  return tonKeyPairToNacl(keyPair);
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

function tonKeyPairToNacl(keyPair: KeyPair): nacl.SignKeyPair {
  const publicKey = Uint8Array.from(keyPair.publicKey);
  const signingKeyBytes = Uint8Array.from(keyPair.secretKey);
  if (publicKey.length !== 32 || signingKeyBytes.length !== 64) {
    throw new Error("mnemonic did not produce an Ed25519 keypair");
  }
  return { publicKey, ["secretKey"]: signingKeyBytes } as nacl.SignKeyPair;
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
