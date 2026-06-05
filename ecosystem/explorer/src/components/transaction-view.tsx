"use client";

import Link from "next/link";
import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertCircle, ArrowRight, CheckCircle2 } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { HashText } from "@/components/hash-text";
import { JsonBlock } from "@/components/json-block";
import { StatusBadge } from "@/components/status-badge";
import { getTransaction } from "@/lib/api";
import { formatAmount, formatUnixTime, shortHash } from "@/lib/format";
import { useExplorerSettings } from "@/lib/settings";

export function TransactionView({ hash }: { hash: string }) {
  const apiBase = useExplorerSettings((state) => state.apiBase);
  const transaction = useQuery({
    queryKey: ["transaction", apiBase, hash],
    queryFn: () => getTransaction(apiBase, hash),
  });

  if (transaction.isPending || (!transaction.data && !transaction.error)) {
    return <Skeleton className="h-[36rem] w-full" />;
  }

  if (transaction.error) {
    return (
      <Alert className="border-red-500/30 bg-red-950/30">
        <AlertCircle className="h-4 w-4" />
        <AlertTitle>Transaction lookup failed</AlertTitle>
        <AlertDescription>
          {transaction.error &&
          typeof transaction.error === "object" &&
          "message" in transaction.error
            ? String(transaction.error.message)
            : "request failed"}
        </AlertDescription>
      </Alert>
    );
  }

  const data = transaction.data;
  if (!data) return <Skeleton className="h-[36rem] w-full" />;
  const from = data.participants.find((item) => item.role === "from");
  const to = data.participants.find((item) => item.role !== "from");

  return (
    <div className="space-y-5">
      <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
        <CardContent className="flex flex-col gap-4 p-5 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-emerald-300">
              <CheckCircle2 className="h-5 w-5" />
              <span className="font-semibold">Confirmed transaction</span>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-400">
              {from ? (
                <Link href={`/account/${from.raw_address}`}>
                  <HashText value={from.user_friendly_address} />
                </Link>
              ) : (
                <span>system</span>
              )}
              <ArrowRight className="h-4 w-4" />
              {to ? (
                <Link href={`/account/${to.raw_address}`}>
                  <HashText value={to.user_friendly_address} />
                </Link>
              ) : (
                <span>external</span>
              )}
            </div>
          </div>
          <div className="text-left md:text-right">
            <div className="text-sm text-zinc-300">
              {formatUnixTime(data.timestamp)}
            </div>
            <div className="mt-2">
              <StatusBadge status={data.status} />
            </div>
          </div>
        </CardContent>
      </Card>

      <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
        <CardHeader>
          <CardTitle className="text-base text-zinc-50">Event Overview</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-4">
            <Metric label="Action" value={data.kind} />
            <Metric
              label="Route"
              value={`${shortHash(from?.account_id)} -> ${shortHash(to?.account_id)}`}
            />
            <Metric label="Value" value={formatAmount(data.amount)} />
            <Metric label="Gas charged" value={formatAmount(data.gas_charged)} />
          </div>
          <div className="mt-6 flex items-center justify-center">
            <div className="grid min-h-28 w-full max-w-xl grid-cols-[1fr_auto_1fr] items-center gap-3">
              <FlowNode label="A" value={from?.user_friendly_address ?? "system"} />
              <div className="flex flex-col items-center gap-2 text-xs text-zinc-400">
                <ArrowRight className="h-5 w-5 text-zinc-500" />
                <span>{data.kind}</span>
                <span>{formatAmount(data.amount)}</span>
              </div>
              <FlowNode label="B" value={to?.user_friendly_address ?? "external"} />
            </div>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-5 lg:grid-cols-2">
        <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
          <CardHeader>
            <CardTitle className="text-base text-zinc-50">Transaction</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Detail label="Tx hash" value={<HashText full value={data.tx_hash} />} />
            <Detail label="Block" value={`#${data.block_height} / ${data.tx_index}`} />
            <Detail label="Block hash" value={<HashText value={data.block_hash} />} />
            <Detail label="Chain" value={data.chain_id} />
            <Detail label="Nonce" value={String(data.nonce)} />
            <Detail label="Gas limit" value={String(data.gas_limit)} />
            <Detail label="Max gas price" value={formatAmount(data.max_gas_price)} />
            <Detail label="Reason" value={data.reason ?? "-"} />
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
          <CardHeader>
            <CardTitle className="text-base text-zinc-50">Commitments</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Detail label="State root" value={<HashText value={data.state_root} />} />
            <Detail label="Tx root" value={<HashText value={data.tx_root} />} />
            <Detail label="Receipt root" value={<HashText value={data.receipt_root} />} />
            <Detail
              label="Withdrawal root"
              value={<HashText value={data.withdrawal_root} />}
            />
            <Detail label="Data hash" value={<HashText value={data.data_hash} />} />
            <Detail
              label="Withdrawal id"
              value={<HashText value={data.withdrawal_id} />}
            />
          </CardContent>
        </Card>
      </div>

      <Card className="border-white/10 bg-white/[0.06] shadow-xl shadow-black/10 backdrop-blur">
        <CardHeader>
          <CardTitle className="text-base text-zinc-50">Raw Data</CardTitle>
        </CardHeader>
        <CardContent>
          <Tabs defaultValue="transaction">
            <TabsList>
              <TabsTrigger value="transaction">Transaction</TabsTrigger>
              <TabsTrigger value="receipt">Receipt</TabsTrigger>
            </TabsList>
            <TabsContent value="transaction">
              <JsonBlock value={data.raw_transaction} />
            </TabsContent>
            <TabsContent value="receipt">
              <JsonBlock value={data.raw_receipt} />
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-white/10 bg-black/20 p-3">
      <div className="text-xs text-zinc-500">{label}</div>
      <div className="mt-2 min-h-5 text-sm text-zinc-100">{value}</div>
    </div>
  );
}

function Detail({
  label,
  value,
}: {
  label: string;
  value: string | ReactNode;
}) {
  return (
    <div>
      <div className="mb-2 flex items-center justify-between gap-4">
        <span className="text-xs text-zinc-500">{label}</span>
        <span className="min-w-0 text-right text-sm text-zinc-100">{value}</span>
      </div>
      <Separator className="bg-white/10" />
    </div>
  );
}

function FlowNode({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 items-center gap-3 rounded-md border border-white/10 bg-black/20 p-3">
      <Badge variant="secondary">{label}</Badge>
      <HashText value={value} />
    </div>
  );
}
