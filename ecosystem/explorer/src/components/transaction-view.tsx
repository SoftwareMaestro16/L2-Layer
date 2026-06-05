"use client";

import { useQuery } from "@tanstack/react-query";
import { ArrowRight, CheckCircle2, CircleAlert } from "lucide-react";
import { useState } from "react";
import { HashText } from "@/components/hash-text";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { fetchTransaction } from "@/lib/api";
import { formatBaseUnits, formatTime, shortHash } from "@/lib/format";
import type { FlowNode } from "@/lib/schemas";
import { useApiStore } from "@/lib/store";

export function TransactionView({ hash }: { hash: string }) {
  const apiBase = useApiStore((state) => state.apiBase);
  const tx = useQuery({ queryKey: ["tx", apiBase, hash], queryFn: () => fetchTransaction(apiBase, hash) });
  const [selectedId, setSelectedId] = useState<string | null>(null);

  if (tx.isLoading) return <Card><CardContent>Loading transaction...</CardContent></Card>;
  if (tx.isError) return <Card><CardContent>Transaction lookup failed.</CardContent></Card>;

  const data = tx.data;
  if (!data) return <Card><CardContent>Transaction not found.</CardContent></Card>;
  const selected = data.flow.find((node) => node.id === selectedId) ?? data.flow[0];

  return (
    <div className="space-y-5">
      <Card className="bg-[var(--panel-strong)]">
        <CardContent className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              {data.status === "applied" ? <CheckCircle2 className="h-5 w-5 text-emerald-300" /> : <CircleAlert className="h-5 w-5 text-red-300" />}
              <h1 className="text-xl font-bold">{data.status === "applied" ? "Confirmed transaction" : "Rejected transaction"}</h1>
              <Badge>{data.kind}</Badge>
            </div>
            <p className="mt-2 text-sm text-zinc-400"><HashText value={data.tx_hash} /> at block #{data.block_height} / {data.tx_index}</p>
          </div>
          <p className="text-sm text-zinc-300">{formatTime(data.timestamp)}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Event Overview</CardTitle></CardHeader>
        <CardContent>
          <div className="mb-8 grid gap-3 md:grid-cols-4">
            <Metric label="Action" value={data.operation ?? data.kind} />
            <Metric label="Value" value={data.amount ? `${formatBaseUnits(data.amount)} asset ${data.asset_id ?? 0}` : "-"} />
            <Metric label="Gas charged" value={data.gas_charged ?? "0"} />
            <Metric label="Status" value={data.status} />
          </div>
          <div className="flex flex-wrap items-center justify-center gap-3">
            {data.flow.map((node, index) => (
              <div key={node.id} className="flex items-center gap-3">
                <button
                  className={`min-w-44 rounded-lg border px-4 py-3 text-left transition ${selected?.id === node.id ? "border-violet-300 bg-violet-500/20" : "border-white/10 bg-white/[0.05] hover:bg-white/[0.09]"}`}
                  onClick={() => setSelectedId(node.id)}
                >
                  <p className="text-sm font-semibold">{node.label}</p>
                  <p className="mt-1 truncate text-xs text-zinc-400">{node.user_friendly_address ? shortHash(node.user_friendly_address) : node.amount ? formatBaseUnits(node.amount) : node.status ?? node.role}</p>
                </button>
                {index < data.flow.length - 1 ? <ArrowRight className="h-5 w-5 text-zinc-500" /> : null}
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-5 lg:grid-cols-2">
        <Card>
          <CardHeader><CardTitle>Selected Flow Node</CardTitle></CardHeader>
          <CardContent>{selected ? <FlowDetails node={selected} /> : "No flow node selected."}</CardContent>
        </Card>
        <Card>
          <CardHeader><CardTitle>Commitments</CardTitle></CardHeader>
          <CardContent className="grid gap-3 text-sm">
            <Line label="Tx root" value={data.tx_root} />
            <Line label="Receipt root" value={data.receipt_root} />
            <Line label="Withdrawal root" value={data.withdrawal_root} />
            <Line label="State root" value={data.state_root} />
            <Line label="Data hash" value={data.data_hash} />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader><CardTitle>Raw Transaction / Receipt</CardTitle></CardHeader>
        <CardContent><pre className="max-h-96 overflow-auto rounded-lg bg-black/30 p-4 text-xs">{JSON.stringify({ raw_transaction: data.raw_transaction, raw_receipt: data.raw_receipt }, null, 2)}</pre></CardContent>
      </Card>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div className="rounded-lg border border-white/10 bg-white/[0.04] p-4"><p className="text-xs text-zinc-500">{label}</p><p className="mt-2 truncate font-semibold">{value}</p></div>;
}

function Line({ label, value }: { label: string; value: string }) {
  return <div className="flex items-center justify-between gap-3 border-b border-white/10 pb-2"><span className="text-zinc-500">{label}</span><HashText value={value} /></div>;
}

function FlowDetails({ node }: { node: FlowNode }) {
  return (
    <dl className="grid gap-3 text-sm">
      <LineValue label="Role" value={node.role} />
      {node.user_friendly_address ? <LineValue label="Address" value={node.user_friendly_address} /> : null}
      {node.amount ? <LineValue label="Amount" value={`${formatBaseUnits(node.amount)} asset ${node.asset_id ?? 0}`} /> : null}
      {node.gas_charged ? <LineValue label="Gas" value={node.gas_charged} /> : null}
      {node.status ? <LineValue label="Status" value={node.status} /> : null}
      {node.reason ? <LineValue label="Reason" value={node.reason} /> : null}
      <pre className="mt-2 overflow-auto rounded-lg bg-black/30 p-3 text-xs">{JSON.stringify(node.details, null, 2)}</pre>
    </dl>
  );
}

function LineValue({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-zinc-500">{label}</dt><dd className="mt-1 break-all font-mono text-zinc-100">{value}</dd></div>;
}
