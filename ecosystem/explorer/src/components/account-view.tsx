"use client";

import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { QRCodeSVG } from "qrcode.react";
import { Code2, Gem, Layers3, Play, Send, Terminal } from "lucide-react";
import { useMemo, useState } from "react";
import { HashText } from "@/components/hash-text";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { fetchAccount, fetchAccountAssets, fetchAccountCode, fetchTransactions } from "@/lib/api";
import { enwalletSendLink, formatBaseUnits, formatTime } from "@/lib/format";
import { useApiStore } from "@/lib/store";

const tabs = ["History", "Raw Transactions", "Code", "Methods", "Send Message", "Jettons", "Collectibles"] as const;
type AccountTab = (typeof tabs)[number];

export function AccountView({ address }: { address: string }) {
  const apiBase = useApiStore((state) => state.apiBase);
  const [tab, setTab] = useState<AccountTab>("History");
  const account = useQuery({ queryKey: ["account", apiBase, address], queryFn: () => fetchAccount(apiBase, address) });
  const assets = useQuery({ queryKey: ["account-assets", apiBase, address], queryFn: () => fetchAccountAssets(apiBase, address) });
  const txs = useInfiniteQuery({
    queryKey: ["account-transactions", apiBase, address],
    queryFn: ({ pageParam }) => fetchTransactions(apiBase, address, pageParam),
    getNextPageParam: (page) => page.next_cursor,
    initialPageParam: null as { before_height: number; before_index: number } | null
  });

  const rows = useMemo(() => txs.data?.pages.flatMap((page) => page.items) ?? [], [txs.data]);

  if (account.isLoading) return <Card><CardContent>Loading account...</CardContent></Card>;
  if (account.isError) return <Card><CardContent>Account lookup failed.</CardContent></Card>;

  const current = account.data;
  if (!current) return <Card><CardContent>Account not found.</CardContent></Card>;

  return (
    <div className="space-y-5">
      <Card className="bg-[var(--panel-strong)]">
        <CardContent className="grid gap-5 md:grid-cols-[1fr_auto]">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="truncate text-2xl font-bold">Account <HashText value={current.user_friendly_address} /></h1>
              <Badge>{current.status}</Badge>
              {current.interfaces.map((item) => <Badge key={item.id}>{item.label}</Badge>)}
            </div>
            <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
              <Field label="Raw address" value={current.raw_address} />
              <Field label="Nonce" value={String(current.nonce)} />
              <Field label="Last LT" value={String(current.last_lt)} />
              <Field label="Code hash" value={current.code_hash} mono />
              <Field label="Data hash" value={current.data_hash} mono />
              <Field label="Storage root" value={current.storage_root} mono />
            </dl>
          </div>
          <div className="grid min-w-40 gap-2 rounded-lg border border-white/10 bg-white/[0.05] p-4 text-right">
            <span className="text-sm text-zinc-400">Primary balance</span>
            <strong className="text-2xl">{formatBaseUnits(current.balances.find((b) => b.asset_id === 0)?.amount ?? "0")} ENT</strong>
          </div>
        </CardContent>
      </Card>

      <Card>
        <div className="flex gap-5 overflow-x-auto border-b border-white/10 px-5">
          {tabs.map((item) => (
            <button
              key={item}
              className={`h-12 whitespace-nowrap border-b-2 text-sm font-semibold ${tab === item ? "border-white text-white" : "border-transparent text-zinc-400"}`}
              onClick={() => setTab(item)}
            >
              {item}
            </button>
          ))}
        </div>
        {tab === "History" ? <HistoryTab rows={rows} loadMore={() => txs.fetchNextPage()} hasMore={txs.hasNextPage} /> : null}
        {tab === "Raw Transactions" ? <RawTransactions rows={rows} /> : null}
        {tab === "Code" ? <CodeTab apiBase={apiBase} address={address} /> : null}
        {tab === "Methods" ? <MethodsTab apiBase={apiBase} address={address} /> : null}
        {tab === "Send Message" ? <SendMessageTab address={current.raw_address} /> : null}
        {tab === "Jettons" ? <JettonsTab tokens={assets.data?.tokens ?? []} /> : null}
        {tab === "Collectibles" ? <CollectiblesTab items={assets.data?.collectibles ?? []} /> : null}
      </Card>
    </div>
  );
}

function Field({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="min-w-0 rounded-lg border border-white/10 bg-white/[0.04] p-3">
      <dt className="text-xs text-zinc-500">{label}</dt>
      <dd className={`mt-1 truncate text-sm ${mono ? "font-mono" : "font-semibold"}`}>{value}</dd>
    </div>
  );
}

function HistoryTab({ rows, loadMore, hasMore }: { rows: Awaited<ReturnType<typeof fetchTransactions>>["items"]; loadMore: () => void; hasMore?: boolean }) {
  return (
    <CardContent className="p-0">
      {rows.map((tx) => (
        <a key={`${tx.block_height}-${tx.tx_index}`} href={`/transaction/${tx.tx_hash}`} className="grid gap-3 border-b border-white/10 px-5 py-3 text-sm hover:bg-white/[0.04] md:grid-cols-[160px_220px_1fr_160px]">
          <span className="text-zinc-400">{formatTime(tx.timestamp)}</span>
          <span className="font-semibold">{txTitle(tx.kind, tx.direction)}</span>
          <span><HashText value={tx.tx_hash} /></span>
          <span className={tx.direction === "in" ? "text-emerald-300" : "text-white"}>{tx.amount ? `${tx.direction === "in" ? "+" : "-"}${formatBaseUnits(tx.amount)} asset ${tx.asset_id ?? 0}` : "-"}</span>
        </a>
      ))}
      <div className="p-4">{hasMore ? <Button variant="secondary" onClick={loadMore}>Load more</Button> : <span className="text-sm text-zinc-500">No more transactions.</span>}</div>
    </CardContent>
  );
}

function RawTransactions({ rows }: { rows: Awaited<ReturnType<typeof fetchTransactions>>["items"] }) {
  return <CardContent><pre className="overflow-auto rounded-lg bg-black/30 p-4 text-xs">{JSON.stringify(rows, null, 2)}</pre></CardContent>;
}

function CodeTab({ apiBase, address }: { apiBase: string; address: string }) {
  const code = useQuery({ queryKey: ["account-code", apiBase, address], queryFn: () => fetchAccountCode(apiBase, address) });
  const [section, setSection] = useState<"bytecode" | "raw_data">("bytecode");
  const [format, setFormat] = useState<"hex" | "cells" | "base64" | "hash">("hex");
  if (code.isLoading) return <CardContent>Loading contract code...</CardContent>;
  if (code.isError) return <CardContent>Contract code not found.</CardContent>;
  const codeData = code.data;
  if (!codeData) return <CardContent>Contract code not found.</CardContent>;
  const active = codeData[section];
  return (
    <CardContent className="grid gap-4 lg:grid-cols-[170px_1fr]">
      <div className="space-y-2">
        <Button className="w-full justify-start" variant={section === "bytecode" ? "default" : "secondary"} onClick={() => setSection("bytecode")}><Code2 className="h-4 w-4" />Bytecode</Button>
        <Button className="w-full justify-start" variant={section === "raw_data" ? "default" : "secondary"} onClick={() => setSection("raw_data")}><DatabaseIcon />Raw data</Button>
      </div>
      <div className="min-w-0 space-y-4">
        {codeData.source.status === "verified" ? <VerifiedSource files={codeData.source.files} /> : <UnverifiedSource />}
        <div className="flex flex-wrap gap-2">
          {(["hex", "cells", "base64", "hash"] as const).map((item) => <Button key={item} size="sm" variant={format === item ? "default" : "secondary"} onClick={() => setFormat(item)}>{item === "hash" ? "Hex hash" : item}</Button>)}
        </div>
        <pre className="max-h-96 overflow-auto rounded-lg border border-white/10 bg-black/30 p-4 text-xs leading-6">{format === "hex" ? active.hex : format === "base64" ? active.base64 : format === "hash" ? active.hex_hash : JSON.stringify(active.cells, null, 2)}</pre>
      </div>
    </CardContent>
  );
}

function DatabaseIcon() {
  return <Terminal className="h-4 w-4" />;
}

function UnverifiedSource() {
  return <div className="rounded-lg border border-white/10 bg-white/[0.04] p-4"><p className="font-semibold">Sources not verified</p><p className="text-sm text-zinc-400">You can add the source code on the <a className="text-sky-300" href="/verifier">verifier</a></p></div>;
}

function VerifiedSource({ files }: { files: Array<{ path: string; content: string }> }) {
  return <div className="space-y-3">{files.map((file) => <div key={file.path}><p className="mb-2 text-sm font-semibold text-emerald-300">{file.path}</p><pre className="max-h-72 overflow-auto rounded-lg bg-black/30 p-4 text-xs">{file.content}</pre></div>)}</div>;
}

function MethodsTab({ apiBase, address }: { apiBase: string; address: string }) {
  const [method, setMethod] = useState("seqno");
  const [result, setResult] = useState("");
  async function execute() {
    const response = await fetch(`${apiBase}/v1/contract/${encodeURIComponent(address)}/get/${encodeURIComponent(method)}`);
    setResult(await response.text());
  }
  return (
    <CardContent className="grid gap-4 md:grid-cols-[240px_1fr]">
      <div className="space-y-2">{["get_public_key", "is_signature_allowed", "seqno"].map((item) => <Button key={item} className="w-full justify-start" variant={method === item ? "default" : "ghost"} onClick={() => setMethod(item)}>{item}</Button>)}</div>
      <div className="space-y-3"><Input value={method} onChange={(e) => setMethod(e.currentTarget.value)} /><Button onClick={execute}><Play className="h-4 w-4" />Execute</Button><Textarea readOnly value={result} placeholder="Getter result" /></div>
    </CardContent>
  );
}

function SendMessageTab({ address }: { address: string }) {
  const [amount, setAmount] = useState("");
  const link = enwalletSendLink(address, 0, amount || undefined);
  return (
    <CardContent className="grid gap-5 md:grid-cols-[220px_1fr]">
      <div className="rounded-lg bg-white p-4"><QRCodeSVG value={link} size={188} /></div>
      <div className="space-y-3"><p className="text-sm text-zinc-400">Scan with EnWallet to prepare an L2 asset transfer.</p><Input value={amount} onChange={(e) => setAmount(e.currentTarget.value)} placeholder="Optional amount in base units" /><a className="inline-flex" href={link}><Button><Send className="h-4 w-4" />Open EnWallet link</Button></a><p className="break-all font-mono text-xs text-zinc-500">{link}</p></div>
    </CardContent>
  );
}

function JettonsTab({ tokens }: { tokens: Array<{ id: string; symbol: string; name: string; amount: string; asset_id: number; decimals: number }> }) {
  return <CardContent className="grid gap-3">{tokens.map((token) => <div key={token.id} className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.04] p-4"><span><Layers3 className="mr-2 inline h-4 w-4 text-violet-300" />{token.name}</span><strong>{formatBaseUnits(token.amount, token.decimals)} {token.symbol}</strong></div>)}</CardContent>;
}

function CollectiblesTab({ items }: { items: Array<{ id: string; name: string; collection: string }> }) {
  return <CardContent>{items.length ? <div className="grid gap-3 sm:grid-cols-3">{items.map((item) => <div key={item.id} className="rounded-lg border border-white/10 p-4"><Gem className="mb-3 h-5 w-5 text-violet-300" /><p className="font-semibold">{item.name}</p><p className="text-sm text-zinc-400">{item.collection}</p></div>)}</div> : <p className="text-sm text-zinc-400">No live L2 NFT indexer data is available for this account yet.</p>}</CardContent>;
}

function txTitle(kind: string, direction: string) {
  if (kind === "call_contract") return "Called contract";
  if (kind === "deploy_contract") return "Contract deploy";
  if (kind === "deposit") return "Received deposit";
  if (kind === "withdraw") return "Withdrawal";
  return direction === "in" ? "Received asset" : "Sent asset";
}
