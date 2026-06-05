import type nacl from "tweetnacl";
import {
  createEnWalletMnemonic,
  enwalletKeyPairFromMnemonic,
  enwalletV5InitialState,
  validateEnWalletMnemonic,
  type EnWalletV5InitialState,
} from "./enwallet.js";
import { deriveAccountId } from "./consensus.js";
import { l2RawAddress, l2UserFriendlyAddress, type Hash32 } from "./address.js";

export * from "./index.js";
export { EntropisClient as BrowserEntropisClient } from "./index.js";

type UIntLike = bigint | number | string;

export interface BrowserWalletAccount {
  recoveryWords: string[];
  keyPair: nacl.SignKeyPair;
  ownerAccountId: Hash32;
  walletAccountId: Hash32;
  rawAddress: string;
  userFriendlyAddress: string;
  initialState: EnWalletV5InitialState;
}

export interface BrowserWalletAccountOptions {
  walletId?: UIntLike;
}

export async function createEntropisWalletAccount(
  options: BrowserWalletAccountOptions = {},
): Promise<BrowserWalletAccount> {
  const recoveryWords = await createEnWalletMnemonic();
  return importEntropisWalletAccount(recoveryWords, options);
}

export async function importEntropisWalletAccount(
  recoveryWords: string[] | string,
  options: BrowserWalletAccountOptions = {},
): Promise<BrowserWalletAccount> {
  const words = Array.isArray(recoveryWords) ? recoveryWords : recoveryWords.trim().split(/\s+/);
  if (!(await validateEnWalletMnemonic(words))) {
    throw new Error("invalid EnWallet mnemonic");
  }

  const keyPair = await enwalletKeyPairFromMnemonic(words);
  const ownerAccountId = deriveAccountId(keyPair.publicKey);
  const initialState = enwalletV5InitialState({
    publicKey: keyPair.publicKey,
    walletId: options.walletId,
  });
  const walletAccountId = initialState.wallet_account_id;

  return {
    recoveryWords: words,
    keyPair,
    ownerAccountId,
    walletAccountId,
    rawAddress: l2RawAddress(walletAccountId),
    userFriendlyAddress: l2UserFriendlyAddress(walletAccountId),
    initialState,
  };
}
