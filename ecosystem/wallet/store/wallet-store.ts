"use client";

import { create } from "zustand";
import { createMockSession } from "@/lib/mock-data";
import type { MockTransaction, SendTransferInput, WalletSession } from "@/lib/types";

const mockFee = 0.013;

type WalletStore = {
  session: WalletSession | null;
  createWallet: () => void;
  importWallet: () => void;
  lockWallet: () => void;
  sendMockTransfer: (input: SendTransferInput) => { ok: true } | { ok: false; message: string };
};

export const useWalletStore = create<WalletStore>((set, get) => ({
  session: null,
  createWallet: () => set({ session: createMockSession("created") }),
  importWallet: () => set({ session: createMockSession("imported") }),
  lockWallet: () => set({ session: null }),
  sendMockTransfer: (input) => {
    const session = get().session;
    if (!session) {
      return { ok: false, message: "Wallet is locked." };
    }

    const amount = Number(input.amount);
    const total = amount + mockFee;
    if (total > session.balance.amount) {
      return { ok: false, message: "Mock balance is too low for this transfer." };
    }

    const transaction: MockTransaction = {
      id: `tx_mock_${Date.now()}`,
      type: "send",
      status: "pending",
      title: "Pending mock send",
      counterparty: input.recipient,
      amount: -amount,
      fee: mockFee,
      timestamp: new Date().toISOString(),
      memo: input.memo || undefined
    };

    set({
      session: {
        ...session,
        balance: {
          ...session.balance,
          amount: Number((session.balance.amount - total).toFixed(6)),
          fiatValue: Number((session.balance.fiatValue - amount * 0.5).toFixed(2))
        },
        transactions: [transaction, ...session.transactions]
      }
    });

    return { ok: true };
  }
}));
