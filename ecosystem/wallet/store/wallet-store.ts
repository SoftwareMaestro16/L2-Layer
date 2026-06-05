"use client";

import { create } from "zustand";
import {
  identityFromMnemonic,
  parseTokenAmount,
  shortAddress
} from "@/lib/enwallet";
import {
  prepareEntTransfer,
  submitPreparedEntTransfer
} from "@/lib/l2-transfer";
import {
  deleteEncryptedSeed,
  loadEncryptedSeed,
  saveEncryptedSeed,
  storedWalletStatus
} from "@/lib/secure-storage";
import type { SendTransferInput, TransferReview, WalletSession, WalletTransaction } from "@/lib/types";

const SESSION_TIMEOUT_MS = 15 * 60 * 1000;
let lockTimer: number | null = null;

type SendResult =
  | { ok: true; txHash: string; pending: WalletTransaction }
  | { ok: false; message: string };

type OpenResult = { ok: true } | { ok: false; message: string };
type ReviewResult = { ok: true; review: TransferReview } | { ok: false; message: string };

type WalletStore = {
  session: WalletSession | null;
  hasStoredWallet: boolean;
  refreshStoredWallet: () => Promise<void>;
  openStoredWallet: (password: string) => Promise<OpenResult>;
  importWallet: (
    seedPhrase: string,
    password: string,
    createdFrom?: "created" | "imported"
  ) => Promise<OpenResult>;
  lockWallet: () => void;
  forgetWallet: () => Promise<void>;
  prepareTransfer: (input: SendTransferInput) => Promise<ReviewResult>;
  submitReviewedTransfer: (review: TransferReview) => Promise<SendResult>;
};

export const useWalletStore = create<WalletStore>((set, get) => ({
  session: null,
  hasStoredWallet: false,
  refreshStoredWallet: async () => {
    const status = await storedWalletStatus();
    set({ hasStoredWallet: status.encrypted });
  },
  openStoredWallet: async (password) => {
    try {
      const storedWords = await loadEncryptedSeed(password);
      setSession(set, sessionFromWords(storedWords, "imported"));
      return { ok: true };
    } catch (error) {
      set({ session: null, hasStoredWallet: true });
      return { ok: false, message: messageFromError(error, "Could not unlock wallet.") };
    }
  },
  importWallet: async (seedPhrase, password, createdFrom = "imported") => {
    try {
      const session = sessionFromWords(seedPhrase, createdFrom);
      await saveEncryptedSeed(session.recoveryWords, password);
      setSession(set, session);
      return { ok: true };
    } catch (error) {
      return { ok: false, message: messageFromError(error, "Could not store encrypted wallet.") };
    }
  },
  lockWallet: () => {
    clearLockTimer();
    set({ session: null });
  },
  forgetWallet: async () => {
    clearLockTimer();
    await deleteEncryptedSeed();
    set({ session: null, hasStoredWallet: false });
  },
  prepareTransfer: async (input) => {
    const session = get().session;
    if (!session) {
      return { ok: false, message: "Wallet is locked." };
    }

    try {
      const amountBaseUnits = parseTokenAmount(input.amount);
      const review = await prepareEntTransfer({
        recoveryWords: session.recoveryWords,
        accountId: session.account.accountId,
        recipient: input.recipient,
        amount: input.amount,
        amountBaseUnits
      });
      return { ok: true, review };
    } catch (error) {
      return { ok: false, message: messageFromError(error, "Could not prepare transfer.") };
    }
  },
  submitReviewedTransfer: async (review) => {
    const session = get().session;
    if (!session) {
      return { ok: false, message: "Wallet is locked." };
    }

    try {
      const txHash = await submitPreparedEntTransfer(review);
      return {
        ok: true,
        txHash,
        pending: review.pending
      };
    } catch (error) {
      return {
        ok: false,
        message: messageFromError(error, "Transfer failed.")
      };
    }
  }
}));

export async function checkStoredWallet(): Promise<boolean> {
  return (await storedWalletStatus()).encrypted;
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

function setSession(
  set: (state: Partial<WalletStore>) => void,
  session: WalletSession
) {
  clearLockTimer();
  set({ session, hasStoredWallet: true });
  lockTimer = window.setTimeout(() => {
    set({ session: null, hasStoredWallet: true });
  }, SESSION_TIMEOUT_MS);
}

function clearLockTimer() {
  if (lockTimer) {
    window.clearTimeout(lockTimer);
    lockTimer = null;
  }
}

function messageFromError(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}
