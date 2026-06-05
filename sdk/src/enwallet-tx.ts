import type nacl from "tweetnacl";
import type { Hash32 } from "./address.js";
import {
  signCallContractTransaction,
  signDeployContractTransaction,
  type CallContractTransactionParams,
  type DeployContractTransactionParams,
  type SignedL2Transaction,
} from "./contracts.js";
import {
  enwalletV5AccountId,
  enwalletV5InitialState,
  enwalletV5SignedExternalBodyBase64,
} from "./enwallet.js";

type UIntLike = bigint | number | string;

export interface EnWalletV5DeployParams
  extends Omit<DeployContractTransactionParams, "contract" | "codeBocBase64" | "dataBocBase64"> {
  walletId?: UIntLike;
  keyPair: nacl.SignKeyPair;
}

export interface EnWalletV5CallParams
  extends Omit<CallContractTransactionParams, "contract" | "bodyBocBase64"> {
  keyPair: nacl.SignKeyPair;
  walletAccountId?: Hash32;
  walletId?: UIntLike;
  walletSeqno: UIntLike;
  walletValidUntil: UIntLike;
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

export function signEnWalletV5CallTransaction(
  params: EnWalletV5CallParams,
): SignedL2Transaction {
  const walletAccountId =
    params.walletAccountId ??
    enwalletV5AccountId({
      publicKey: params.keyPair.publicKey,
      walletId: params.walletId,
    });
  return signCallContractTransaction({
    chainId: params.chainId,
    from: params.from,
    nonce: params.nonce,
    contract: walletAccountId,
    bodyBocBase64: enwalletV5SignedExternalBodyBase64({
      keyPair: params.keyPair,
      walletId: params.walletId,
      validUntil: params.walletValidUntil,
      seqno: params.walletSeqno,
    }),
    gasLimit: params.gasLimit,
    maxGasPrice: params.maxGasPrice,
    validUntilBlock: params.validUntilBlock,
    feeAssetId: params.feeAssetId,
    memoHash: params.memoHash,
    keyPair: params.keyPair,
  });
}
