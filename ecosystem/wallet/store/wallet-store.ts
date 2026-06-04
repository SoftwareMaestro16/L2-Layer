"use client";

import { create } from "zustand";
import {
  identityFromMnemonic,
  parseTokenAmount,
  shortAddress
} from "@/lib/enwallet";
import {
  getAccountNonce,
  pendingTransfer,
  submitEntTransfer
} from "@/lib/l2-api";
import type { SendTransferInput, WalletSession, WalletTransaction } from "@/lib/types";

const STORAGE_KEY = "enwallet.entropis-testnet.mnemonic.v1";

type SendResult =
  | { ok: true; txHash: string; pending: WalletTransaction }
  | { ok: false; message: string };

type WalletStore = {
  session: WalletSession | null;
  hasStoredWallet: boolean;
  openStoredWallet: () => boolean;
  importWallet: (seedPhrase: string, createdFrom?: "created" | "imported") => void;
  lockWallet: () => void;
  forgetWallet: () => void;
  sendTransfer: (input: SendTransferInput) => Promise<SendResult>;
};

export const useWalletStore = create<WalletStore>((set, get) => ({
  session: null,
  hasStoredWallet: false,
  openStoredWallet: () => {
    const storedWords = readStoredWords();
    if (!storedWords) {
      set({ hasStoredWallet: false });
      return false;
    }
    try {
      set({ session: sessionFromWords(storedWords, "imported"), hasStoredWallet: true });
      return true;
    } catch {
      localStorage.removeItem(STORAGE_KEY);
      set({ session: null, hasStoredWallet: false });
      return false;
    }
  },
  importWallet: (seedPhrase, createdFrom = "imported") => {
    const session = sessionFromWords(seedPhrase, createdFrom);
    localStorage.setItem(STORAGE_KEY, session.recoveryWords);
    set({ session, hasStoredWallet: true });
  },
  lockWallet: () => set({ session: null, hasStoredWallet: Boolean(readStoredWords()) }),
  forgetWallet: () => {
    localStorage.removeItem(STORAGE_KEY);
    set({ session: null, hasStoredWallet: false });
  },
  sendTransfer: async (input) => {
    const session = get().session;
    if (!session) {
      return { ok: false, message: "Wallet is locked." };
    }

    try {
      const amountBaseUnits = parseTokenAmount(input.amount);
      const nonce = await getAccountNonce(session.account.accountId);
      const txHash = await submitEntTransfer({
        recoveryWords: session.recoveryWords,
        accountId: session.account.accountId,
        nonce,
        recipient: input.recipient,
        amountBaseUnits
      });

      return {
        ok: true,
        txHash,
        pending: pendingTransfer(txHash, input.recipient, Number(input.amount))
      };
    } catch (error) {
      return {
        ok: false,
        message: error instanceof Error ? error.message : "Transfer failed."
      };
    }
  }
}));

export function checkStoredWallet(): boolean {
  return Boolean(readStoredWords());
}

function sessionFromWords(seedPhrase: string, createdFrom: "created" | "imported"): WalletSession {
  const identity = identityFromMnemonic(seedPhrase);
  return {
    recoveryWords: identity.recoveryWords,
    account: {
      accountId: identity.accountId,
      rawAddress: identity.rawAddress,
      label: "Entropis Account",
      address: identity.friendlyAddress,
      shortAddress: shortAddress(identity.friendlyAddress),
      network: "Entropis testnet",
      createdFrom,
      publicKey: identity.publicKeyHex
    }
  };
}

function readStoredWords(): string | null {
  if (typeof localStorage === "undefined") {
    return null;
  }
  return localStorage.getItem(STORAGE_KEY);
}
