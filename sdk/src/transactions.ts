import nacl from "tweetnacl";
import { deriveAccountId as deriveAccountIdFromBytes, signingPayload } from "./consensus.js";
import type {
  Hash32,
  SignedL2Transaction,
  SigningParams,
  TransferTransactionParams,
  WithdrawTransactionParams,
} from "./types.js";
import {
  hexToBytes,
  normalizeHash32,
  parseTonAddress,
  toDecimalString,
  toPositiveUint,
  toSafeNumber,
  toUint,
} from "./validation.js";

export function signTransaction(
  tx: Omit<SignedL2Transaction, "public_key" | "signature">,
  keyPair: nacl.SignKeyPair,
): SignedL2Transaction {
  const unsigned: SignedL2Transaction = {
    ...tx,
    public_key: Buffer.from(keyPair.publicKey).toString("hex"),
    signature: null,
  };
  const signature = nacl.sign.detached(signingPayload(unsigned), keyPair.secretKey);
  return {
    ...unsigned,
    signature: Buffer.from(signature).toString("hex"),
  };
}

export function accountIdFromPublicKey(publicKey: string | Uint8Array): Hash32 {
  const bytes = typeof publicKey === "string" ? hexToBytes(publicKey, "publicKey") : publicKey;
  return deriveAccountIdFromBytes(bytes);
}

export function accountIdFromKeyPair(keyPair: nacl.SignKeyPair): Hash32 {
  return accountIdFromPublicKey(keyPair.publicKey);
}

export function buildTransferTransaction(params: TransferTransactionParams) {
  return {
    chain_id: params.chainId,
    from: normalizeHash32(params.from),
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      Transfer: {
        to: normalizeHash32(params.to),
        asset_id: toSafeNumber(toUint(params.assetId, "assetId", 32), "assetId"),
        amount: toDecimalString(toPositiveUint(params.amount, "amount", 128)),
      },
    },
    public_key: null,
    signature: null,
  } satisfies SignedL2Transaction;
}

export function signTransferTransaction(params: TransferTransactionParams & SigningParams) {
  return signTransaction(buildTransferTransaction(params), params.keyPair);
}

export function buildWithdrawTransaction(params: WithdrawTransactionParams) {
  parseTonAddress(params.l1Recipient);
  return {
    chain_id: params.chainId,
    from: normalizeHash32(params.from),
    nonce: toSafeNumber(toUint(params.nonce, "nonce", 64), "nonce"),
    gas_limit: toSafeNumber(toUint(params.gasLimit, "gasLimit", 64), "gasLimit"),
    max_gas_price: toDecimalString(toUint(params.maxGasPrice, "maxGasPrice", 128)),
    kind: {
      Withdraw: {
        asset_id: toSafeNumber(toUint(params.assetId, "assetId", 32), "assetId"),
        amount: toDecimalString(toPositiveUint(params.amount, "amount", 120)),
        l1_recipient: params.l1Recipient,
      },
    },
    public_key: null,
    signature: null,
  } satisfies SignedL2Transaction;
}

export function signWithdrawTransaction(params: WithdrawTransactionParams & SigningParams) {
  return signTransaction(buildWithdrawTransaction(params), params.keyPair);
}
