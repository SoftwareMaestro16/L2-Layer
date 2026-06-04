"use client";

import Link from "next/link";
import type { ReactNode } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { AlertCircle, ArrowDownLeft, ArrowUpRight } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { HashText } from "@/components/hash-text";
import { StatusBadge } from "@/components/status-badge";
import { getAccount, getAccountTransactions } from "@/lib/api";
import { formatAmount, formatUnixTime } from "@/lib/format";
import { useExplorerSettings } from "@/lib/settings";

export function AccountView({ address }: { address: string }) {
  const apiBase = useExplorerSettings((state) => state.apiBase);
  const account = useQuery({
    queryKey: ["account", apiBase, address],
    queryFn: () => getAccount(apiBase, address),
  });
  const transactions = useInfiniteQuery({
    queryKey: ["account-transactions", apiBase, address],
    queryFn: ({ pageParam }) =>
      getAccountTransactions(apiBase, address, pageParam),
    initialPageParam: null as null | {
      before_height: number;
      before_index: number;
    },
    getNextPageParam: (page) => page.next_cursor,
  });

  const rows = transactions.data?.pages.flatMap((page) => page.items) ?? [];

  if (account.isPending || (!account.data && !account.error)) {
    return <AccountSkeleton />;
  }

  if (account.error) {
    return <ExplorerError title="Account lookup failed" error={account.error} />;
  }

  const data = account.data;
  if (!data) return <AccountSkeleton />;

  return (
    <div className="space-y-5">
      <Card className="border-white/10 bg-zinc-900/80">
        <CardHeader className="pb-3">
          <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
            <div className="min-w-0">
              <CardTitle className="text-lg text-zinc-50">Address</CardTitle>
              <div className="mt-2 break-all font-mono text-sm text-zinc-200">
                {data.user_friendly_address}
              </div>
              <div className="mt-2 break-all font-mono text-xs text-zinc-500">
                {data.raw_address}
              </div>
            </div>
            <StatusBadge status={data.status} />
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-4">
            <Metric label="Nonce" value={String(data.nonce)} />
            <Metric label="Last LT" value={String(data.last_lt)} />
            <Metric label="Code hash" value={<HashText value={data.code_hash} />} />
            <Metric label="Storage root" value={<HashText value={data.storage_root} />} />
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-5 lg:grid-cols-[22rem_1fr]">
        <Card className="border-white/10 bg-zinc-900/80">
          <CardHeader>
            <CardTitle className="text-base text-zinc-50">Balances</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {data.balances.length === 0 ? (
              <div className="text-sm text-zinc-500">No balances</div>
            ) : (
              data.balances.map((balance) => (
                <div
                  className="flex items-center justify-between rounded-md border border-white/10 bg-black/20 px-3 py-2"
                  key={balance.asset_id}
                >
                  <Badge variant="secondary">asset {balance.asset_id}</Badge>
                  <span className="font-mono text-sm">
                    {formatAmount(balance.amount)}
                  </span>
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card className="border-white/10 bg-zinc-900/80">
          <CardHeader>
            <CardTitle className="text-base text-zinc-50">
              Transaction History
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Type</TableHead>
                    <TableHead>Hash</TableHead>
                    <TableHead>Amount</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Time</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map((row) => (
                    <TableRow key={`${row.block_height}-${row.tx_index}`}>
                      <TableCell className="min-w-36">
                        <div className="flex items-center gap-2">
                          {row.direction === "out" ? (
                            <ArrowUpRight className="h-4 w-4 text-amber-300" />
                          ) : (
                            <ArrowDownLeft className="h-4 w-4 text-emerald-300" />
                          )}
                          <span>{row.kind}</span>
                        </div>
                      </TableCell>
                      <TableCell>
                        <Link href={`/transaction/${row.tx_hash}`}>
                          <HashText value={row.tx_hash} />
                        </Link>
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {formatAmount(row.amount)}
                        {row.asset_id !== null ? ` / ${row.asset_id}` : ""}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={row.status} />
                      </TableCell>
                      <TableCell className="min-w-44 text-xs text-zinc-400">
                        {formatUnixTime(row.timestamp)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>

            {transactions.isLoading ? (
              <Skeleton className="mt-4 h-10 w-full" />
            ) : rows.length === 0 ? (
              <div className="mt-4 text-sm text-zinc-500">No transactions</div>
            ) : null}

            <div className="mt-4 flex justify-end">
              <Button
                className="h-10 min-w-32"
                disabled={
                  !transactions.hasNextPage || transactions.isFetchingNextPage
                }
                onClick={() => transactions.fetchNextPage()}
                variant="secondary"
              >
                {transactions.isFetchingNextPage ? "Loading" : "Load more"}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function Metric({
  label,
  value,
}: {
  label: string;
  value: string | ReactNode;
}) {
  return (
    <div className="rounded-md border border-white/10 bg-black/20 p-3">
      <div className="text-xs text-zinc-500">{label}</div>
      <div className="mt-2 min-h-5 text-sm text-zinc-100">{value}</div>
    </div>
  );
}

function AccountSkeleton() {
  return (
    <div className="space-y-5">
      <Skeleton className="h-40 w-full" />
      <Skeleton className="h-96 w-full" />
    </div>
  );
}

function ExplorerError({ title, error }: { title: string; error: unknown }) {
  const message =
    error && typeof error === "object" && "message" in error
      ? String(error.message)
      : "request failed";
  return (
    <Alert className="border-red-500/30 bg-red-950/30">
      <AlertCircle className="h-4 w-4" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
    </Alert>
  );
}
