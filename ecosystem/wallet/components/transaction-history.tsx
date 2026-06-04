"use client";

import { ArrowDownLeft, ArrowUpRight, Filter, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { formatDateTime, formatEnt } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { WalletTransaction } from "@/lib/types";

type FilterValue = "all" | WalletTransaction["type"];

export function TransactionHistory({ transactions }: { transactions: WalletTransaction[] }) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterValue>("all");

  const visibleTransactions = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return transactions.filter((transaction) => {
      const matchesFilter = filter === "all" || transaction.type === filter;
      const matchesQuery =
        !normalized ||
        transaction.title.toLowerCase().includes(normalized) ||
        transaction.counterparty.toLowerCase().includes(normalized) ||
        transaction.id.toLowerCase().includes(normalized) ||
        Boolean(transaction.memo?.toLowerCase().includes(normalized));
      return matchesFilter && matchesQuery;
    });
  }, [filter, query, transactions]);

  return (
    <Card className="min-w-0">
      <CardHeader className="gap-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <CardTitle>Transaction history</CardTitle>
          <div className="flex flex-wrap gap-2">
            {(["all", "send", "receive", "deposit", "withdraw"] as FilterValue[]).map((value) => (
              <Button
                key={value}
                size="sm"
                variant={filter === value ? "default" : "outline"}
                onClick={() => setFilter(value)}
              >
                {value === "all" ? <Filter className="h-3.5 w-3.5" /> : null}
                {value}
              </Button>
            ))}
          </div>
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            className="pl-9"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search tx, address, or label"
          />
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {visibleTransactions.map((transaction) => {
          const outgoing = transaction.amount < 0;
          const Icon = outgoing ? ArrowUpRight : ArrowDownLeft;
          return (
            <div
              key={transaction.id}
              className="grid gap-3 rounded-lg border bg-card p-3 sm:grid-cols-[auto_1fr_auto] sm:items-center"
            >
              <div
                className={cn(
                  "flex h-10 w-10 items-center justify-center rounded-md",
                  outgoing
                    ? "bg-violet-50 text-violet-700 dark:bg-violet-500/15 dark:text-violet-100"
                    : "bg-blue-50 text-blue-700 dark:bg-blue-500/15 dark:text-blue-100"
                )}
              >
                <Icon className="h-5 w-5" />
              </div>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-semibold">{transaction.title}</p>
                  <Badge variant={transaction.status === "pending" ? "warning" : "secondary"}>
                    {transaction.status}
                  </Badge>
                </div>
                <p className="mt-1 truncate text-sm text-muted-foreground">{transaction.counterparty}</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {formatDateTime(transaction.timestamp)}
                  {transaction.memo ? ` - ${transaction.memo}` : ""}
                </p>
              </div>
              <div className="text-left sm:text-right">
                <p className={cn("font-semibold", outgoing ? "text-foreground" : "text-blue-700 dark:text-blue-200")}>
                  {transaction.amount > 0 ? "+" : ""}
                  {formatEnt(transaction.amount)} {transaction.symbol}
                </p>
                <p className="text-xs text-muted-foreground">fee {formatEnt(transaction.fee)} ENT</p>
              </div>
            </div>
          );
        })}

        {visibleTransactions.length === 0 ? (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No live L2 transactions match this view.
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
