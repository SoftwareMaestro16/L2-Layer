"use client";

import { AuthPanel } from "@/components/auth-panel";
import { WalletShell } from "@/components/wallet-shell";
import { useWalletStore } from "@/store/wallet-store";
import { useEffect } from "react";

export default function Home() {
  const session = useWalletStore((state) => state.session);
  const openStoredWallet = useWalletStore((state) => state.openStoredWallet);

  useEffect(() => {
    openStoredWallet();
  }, [openStoredWallet]);

  return (
    <main className="min-h-screen bg-background text-foreground">
      {session ? <WalletShell /> : <AuthPanel />}
    </main>
  );
}
