"use client";

import { useQuery } from "@tanstack/react-query";
import { Activity, Database, Eye, ShieldCheck } from "lucide-react";
import { ExplorerShell } from "@/components/explorer-shell";
import { LookupForm } from "@/components/lookup-form";
import { StatusBadge } from "@/components/status-badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getHealth } from "@/lib/api";
import { useExplorerSettings } from "@/lib/settings";

export default function Home() {
  const apiBase = useExplorerSettings((state) => state.apiBase);
  const health = useQuery({
    queryKey: ["health", apiBase],
    queryFn: () => getHealth(apiBase),
  });

  return (
    <ExplorerShell>
      <section className="flex min-h-[52vh] items-center justify-center py-10 md:py-16">
        <div className="w-full text-center">
          <div className="mx-auto mb-5 grid h-16 w-16 place-items-center rounded-2xl bg-[linear-gradient(135deg,#2563eb,#7c3aed)] text-white shadow-2xl shadow-violet-500/30">
            <Eye className="h-8 w-8" />
          </div>
          <h1 className="text-4xl font-black text-white md:text-6xl">
            EnWatcher
          </h1>
          <p className="mx-auto mt-4 max-w-2xl text-base leading-7 text-zinc-300 md:text-lg">
            Public Entropis L2 explorer for account balances, raw addresses,
            paginated transaction history, receipts, gas, hashes, and roots.
          </p>
          <div className="mt-8">
            <LookupForm variant="hero" />
          </div>
        </div>
      </section>

      <div className="grid gap-5 md:grid-cols-3">
        <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur md:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-zinc-50">
              <ShieldCheck className="h-5 w-5 text-violet-300" />
              Read-only Explorer
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-3 md:grid-cols-3">
              <HomeMetric label="Mode" value="Read-only" />
              <HomeMetric label="Surface" value="Accounts" />
              <HomeMetric label="History" value="Paginated" />
            </div>
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-zinc-50">
              <Activity className="h-5 w-5 text-sky-300" />
              API
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="break-all font-mono text-xs text-zinc-400">
              {apiBase}
            </div>
            {health.data ? (
              <StatusBadge status={health.data.status} />
            ) : health.error ? (
              <StatusBadge status="unavailable" />
            ) : (
              <StatusBadge status="checking" />
            )}
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur md:col-span-3">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-zinc-50">
              <Database className="h-5 w-5 text-amber-300" />
              Public Data
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid gap-3 md:grid-cols-4">
              <HomeMetric label="Account" value="balances, roots, nonce" />
              <HomeMetric label="Transaction" value="receipt, gas, reason" />
              <HomeMetric label="Route" value="participants, direction" />
              <HomeMetric label="Commitment" value="block and roots" />
            </div>
          </CardContent>
        </Card>
      </div>
    </ExplorerShell>
  );
}

function HomeMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-white/10 bg-black/20 p-3">
      <div className="text-xs text-zinc-500">{label}</div>
      <div className="mt-2 min-h-5 text-sm text-zinc-100">{value}</div>
    </div>
  );
}
