"use client";

import Image from "next/image";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Lock, RefreshCw, Send, WalletCards } from "lucide-react";
import { AssetSections } from "@/components/asset-sections";
import { useMemo, useState } from "react";
import { BalanceCard } from "@/components/balance-card";
import { ReceivePanel } from "@/components/receive-panel";
import { SendDialog } from "@/components/send-dialog";
import { TransactionHistory } from "@/components/transaction-history";
import { ThemeToggle } from "@/components/theme-toggle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchMockNetworkSnapshot } from "@/lib/mock-api";
import { useWalletStore } from "@/store/wallet-store";

export function WalletShell() {
  const session = useWalletStore((state) => state.session);
  const lockWallet = useWalletStore((state) => state.lockWallet);
  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);

  const { data: network, isFetching } = useQuery({
    queryKey: ["mock-network"],
    queryFn: fetchMockNetworkSnapshot
  });

  const totals = useMemo(() => {
    const sent = session?.transactions.filter((tx) => tx.amount < 0).reduce((sum, tx) => sum + Math.abs(tx.amount), 0);
    const received = session?.transactions.filter((tx) => tx.amount > 0).reduce((sum, tx) => sum + tx.amount, 0);
    return {
      sent: sent ?? 0,
      received: received ?? 0
    };
  }, [session?.transactions]);

  if (!session) {
    return null;
  }

  const currentSession = session;

  async function copyAddress() {
    await navigator.clipboard.writeText(currentSession.account.address);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  }

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-7xl flex-col px-4 py-5">
      <header className="flex flex-col gap-4 border-b pb-5 md:flex-row md:items-center md:justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-lg border bg-card">
            <Image src="/entropis.png" alt="Entropis" width={32} height={32} priority />
          </div>
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-xl font-semibold">EnWallet</h1>
              <Badge variant="warning">Mock mode</Badge>
            </div>
            <p className="text-sm text-muted-foreground">{session.account.network}</p>
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={copyAddress}>
            <Copy className="h-4 w-4" />
            {copied ? "Copied" : session.account.shortAddress}
          </Button>
          <Button
            variant="outline"
            onClick={() => queryClient.invalidateQueries({ queryKey: ["mock-network"] })}
            disabled={isFetching}
          >
            <RefreshCw className={isFetching ? "h-4 w-4 animate-spin" : "h-4 w-4"} />
            Refresh
          </Button>
          <ThemeToggle />
          <Button variant="secondary" onClick={lockWallet}>
            <Lock className="h-4 w-4" />
            Lock
          </Button>
        </div>
      </header>

      <section className="grid gap-4 py-5 lg:grid-cols-[1.25fr_0.75fr]">
        <BalanceCard balance={session.balance} />
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="flex items-center gap-2">
              <WalletCards className="h-5 w-5 text-primary" />
              Network snapshot
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 text-sm">
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Status</span>
              <Badge variant="success">{network?.status ?? "loading"}</Badge>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Latest batch</span>
              <strong>{network?.latestBatch ?? "-"}</strong>
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-muted-foreground">Finality</span>
              <strong className="text-right">{network?.finality ?? "-"}</strong>
            </div>
          </CardContent>
        </Card>
      </section>

      <section className="grid flex-1 gap-4 pb-5 lg:grid-cols-[0.85fr_1.15fr]">
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <SendDialog>
              <Button className="h-12 w-full">
                <Send className="h-4 w-4" />
                Send
              </Button>
            </SendDialog>
            <ReceivePanel address={session.account.address} />
          </div>
          <Card>
            <CardHeader>
              <CardTitle>Activity summary</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-3 text-sm">
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Received</span>
                <strong>{totals.received.toLocaleString("en-US")} ENT</strong>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Sent</span>
                <strong>{totals.sent.toLocaleString("en-US")} ENT</strong>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Transactions</span>
                <strong>{session.transactions.length}</strong>
              </div>
            </CardContent>
          </Card>
          <AssetSections tokens={session.tokens} collectibles={session.collectibles} />
        </div>
        <TransactionHistory transactions={session.transactions} />
      </section>
    </div>
  );
}
