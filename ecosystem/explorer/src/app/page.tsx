"use client";

import { useQuery } from "@tanstack/react-query";
import { Activity, Database, Search } from "lucide-react";
import { ExplorerShell } from "@/components/explorer-shell";
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
      <div className="grid gap-5 md:grid-cols-3">
        <Card className="border-white/10 bg-zinc-900/80 md:col-span-2">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-zinc-50">
              <Search className="h-5 w-5 text-emerald-300" />
              Lookup
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

        <Card className="border-white/10 bg-zinc-900/80">
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

        <Card className="border-white/10 bg-zinc-900/80 md:col-span-3">
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
