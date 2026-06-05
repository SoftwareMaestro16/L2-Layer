"use client";

import type { ReactNode } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  Boxes,
  DatabaseZap,
  ExternalLink,
  GitBranch,
  Layers3,
  RefreshCw,
  Search,
  WalletCards,
} from "lucide-react";
import { HashText } from "@/components/hash-text";
import { OperatorPanel } from "@/components/operator-panel";
import { StatusBadge } from "@/components/status-badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  fetchAppJson,
  getBlocks,
  getDeposits,
  getExplorerSummary,
  getReadiness,
} from "@/lib/api";
import { formatAmount, formatUnixTime } from "@/lib/format";
import { useExplorerSettings } from "@/lib/settings";

type FaucetBatchList = {
  batches?: Array<{ id?: string; status?: string; total?: number; created_at?: string }>;
  items?: Array<{ id?: string; status?: string; total?: number; created_at?: string }>;
};

export function PublicDashboard() {
  const apiBase = useExplorerSettings((state) => state.apiBase);
  const summary = useQuery({
    queryKey: ["summary", apiBase],
    queryFn: () => getExplorerSummary(apiBase),
    refetchInterval: 10_000,
  });
  const readiness = useQuery({
    queryKey: ["readyz", apiBase],
    queryFn: () => getReadiness(apiBase),
    refetchInterval: 10_000,
  });
  const blocks = useQuery({
    queryKey: ["blocks", apiBase],
    queryFn: () => getBlocks(apiBase, null, 8),
    refetchInterval: 10_000,
  });
  const deposits = useQuery({
    queryKey: ["deposits", apiBase],
    queryFn: () => getDeposits(apiBase, null, 8),
    refetchInterval: 10_000,
  });
  const faucet = useQuery({
    queryKey: ["faucet-batches"],
    queryFn: () => fetchAppJson<FaucetBatchList>("/api/faucet/batches"),
    retry: false,
    refetchInterval: 15_000,
  });

  const latest = summary.data?.latest_block;
  const latestCommit = summary.data?.latest_batch_commit;
  const latestFinalized = summary.data?.latest_finalized_batch;
  const faucetItems = faucet.data?.batches ?? faucet.data?.items ?? [];

  return (
    <div className="space-y-5">
      <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          icon={<Layers3 className="h-4 w-4" />}
          label="Latest block"
          value={latest ? `#${latest.height}` : "-"}
          detail={latest ? <HashText value={latest.block_hash} /> : "waiting for sequencer"}
        />
        <MetricCard
          icon={<GitBranch className="h-4 w-4" />}
          label="Latest commit"
          value={latestCommit ? `batch ${latestCommit.batch_no}` : "-"}
          detail={latestCommit ? <StatusBadge status={latestCommit.status} /> : "no L1 commit"}
        />
        <MetricCard
          icon={<DatabaseZap className="h-4 w-4" />}
          label="Finality"
          value={latestFinalized ? `batch ${latestFinalized.batch_no}` : "-"}
          detail={latestFinalized ? <StatusBadge status={latestFinalized.status} /> : "not finalized"}
        />
        <MetricCard
          icon={<Boxes className="h-4 w-4" />}
          label="Readiness"
          value={readiness.data?.status ?? "checking"}
          detail={<StatusBadge status={readiness.data?.status ?? "checking"} />}
        />
      </section>

      <section className="grid gap-5 xl:grid-cols-[1.25fr_0.75fr]">
        <Card className="border-white/10 bg-white/[0.05]">
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="flex items-center gap-2 text-base">
              <Layers3 className="h-4 w-4 text-cyan-300" />
              Latest blocks
            </CardTitle>
            <Button size="sm" variant="secondary" onClick={() => blocks.refetch()}>
              <RefreshCw className="h-4 w-4" />
              Refresh
            </Button>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Height</TableHead>
                  <TableHead>Tx</TableHead>
                  <TableHead>DA</TableHead>
                  <TableHead>State root</TableHead>
                  <TableHead>Time</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(blocks.data?.items ?? []).map((block) => (
                  <TableRow key={block.height}>
                    <TableCell>
                      <Link className="font-mono text-cyan-200" href={`/block/${block.height}`}>
                        #{block.height}
                      </Link>
                    </TableCell>
                    <TableCell>{block.tx_count}</TableCell>
                    <TableCell>
                      <Link href={`/da/${block.height}`}>
                        <HashText value={block.data_hash} />
                      </Link>
                    </TableCell>
                    <TableCell>
                      <HashText value={block.state_root} />
                    </TableCell>
                    <TableCell className="min-w-44 text-xs text-zinc-400">
                      {formatUnixTime(block.timestamp)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            {blocks.data?.items.length === 0 ? <EmptyState label="No committed blocks yet" /> : null}
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-white/[0.05]">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <WalletCards className="h-4 w-4 text-emerald-300" />
              Bridge deposits
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {(deposits.data?.items ?? []).map((item) => (
              <Link
                className="block rounded-md border border-white/10 bg-black/20 p-3 hover:bg-white/[0.06]"
                href={`/deposit/${item.deposit.deposit_id}`}
                key={item.deposit.deposit_id}
              >
                <div className="flex items-center justify-between gap-3">
                  <HashText value={item.deposit.deposit_id} />
                  <StatusBadge status={item.status} />
                </div>
                <div className="mt-2 flex items-center justify-between text-xs text-zinc-400">
                  <span>asset {item.deposit.asset_id}</span>
                  <span>{formatAmount(item.deposit.amount)}</span>
                </div>
              </Link>
            ))}
            {deposits.data?.items.length === 0 ? <EmptyState label="No indexed deposits yet" /> : null}
          </CardContent>
        </Card>
      </section>

      <section className="grid gap-5 xl:grid-cols-2">
        <Card className="border-white/10 bg-white/[0.05]">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Search className="h-4 w-4 text-amber-300" />
              Direct inspection
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 sm:grid-cols-2">
            <Shortcut href="/contract" label="Contract state" />
            <Shortcut href="/deposit" label="Deposit status" />
            <Shortcut href="/withdrawal" label="Withdrawal status" />
            <Shortcut href="/da" label="DA payload" />
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-white/[0.05]">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <ExternalLink className="h-4 w-4 text-cyan-300" />
              Faucet batches
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {faucet.isError ? <EmptyState label="Faucet backend is not configured" /> : null}
            {faucetItems.slice(0, 5).map((batch, index) => (
              <div className="rounded-md border border-white/10 bg-black/20 p-3" key={batch.id ?? index}>
                <div className="flex items-center justify-between">
                  <span className="font-mono text-xs text-zinc-200">{batch.id ?? `batch-${index}`}</span>
                  <StatusBadge status={batch.status ?? "unknown"} />
                </div>
                <div className="mt-2 text-xs text-zinc-400">claims: {batch.total ?? "-"}</div>
              </div>
            ))}
          </CardContent>
        </Card>
      </section>

      <OperatorPanel />
    </div>
  );
}

function MetricCard({
  icon,
  label,
  value,
  detail,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  detail: ReactNode;
}) {
  return (
    <Card className="border-white/10 bg-white/[0.05]">
      <CardContent className="p-4">
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          {icon}
          {label}
        </div>
        <div className="mt-3 text-2xl font-semibold text-zinc-50">{value}</div>
        <div className="mt-2 min-h-5 text-xs text-zinc-400">{detail}</div>
      </CardContent>
    </Card>
  );
}

function Shortcut({ href, label }: { href: string; label: string }) {
  return (
    <Link className="rounded-md border border-white/10 bg-black/20 px-3 py-2 text-sm hover:bg-white/[0.06]" href={href}>
      {label}
    </Link>
  );
}

function EmptyState({ label }: { label: string }) {
  return <div className="rounded-md border border-dashed border-white/10 p-4 text-sm text-zinc-500">{label}</div>;
}
