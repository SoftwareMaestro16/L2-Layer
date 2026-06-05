"use client";

import { useQuery } from "@tanstack/react-query";
import { Activity, Box, Coins, Database, ShieldCheck, Users } from "lucide-react";
import { LookupForm } from "@/components/lookup-form";
import { Card, CardContent } from "@/components/ui/card";
import { fetchSummary } from "@/lib/api";
import { formatBaseUnits } from "@/lib/format";
import { useApiStore } from "@/lib/store";

const stats = [
  ["Blocks", "block_count", Box],
  ["Transactions", "transaction_count", Activity],
  ["Supply", "live_ent_supply", Coins],
  ["Deposits", "deposit_count", Database],
  ["Withdrawals", "withdrawal_count", ShieldCheck],
  ["Accounts", "live_account_count", Users]
] as const;

export default function HomePage() {
  const apiBase = useApiStore((state) => state.apiBase);
  const summary = useQuery({ queryKey: ["summary", apiBase], queryFn: () => fetchSummary(apiBase), refetchInterval: 10_000 });

  return (
    <main className="mx-auto flex min-h-dvh w-full max-w-7xl flex-col px-4 py-6">
      <section className="flex flex-1 flex-col items-center justify-center py-16 text-center">
        <div className="mb-6 rounded-lg border border-white/10 bg-white/[0.08] px-4 py-2 text-sm text-violet-100">
          Public Entropis L2 testnet explorer
        </div>
        <h1 className="text-5xl font-black tracking-normal text-white sm:text-7xl">EnWatcher</h1>
        <p className="mt-4 max-w-2xl text-base leading-7 text-zinc-300">
          Search accounts, transactions, contract bytecode, raw state, verifier sources, and L2 assets.
        </p>
        <div className="mt-8 w-full">
          <LookupForm variant="hero" />
        </div>
      </section>

      <section className="grid gap-3 pb-8 sm:grid-cols-2 lg:grid-cols-6">
        {stats.map(([label, key, Icon]) => (
          <Card key={key}>
            <CardContent className="p-4">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm text-zinc-400">{label}</span>
                <Icon className="h-4 w-4 text-violet-300" />
              </div>
              <p className="mt-3 truncate text-2xl font-bold">
                {summary.data ? statValue(summary.data[key]) : summary.isError ? "-" : "..."}
              </p>
            </CardContent>
          </Card>
        ))}
      </section>
    </main>
  );
}

function statValue(value: number | string): string {
  return typeof value === "string" ? `${formatBaseUnits(value)} ENT` : value.toLocaleString("en-US");
}
