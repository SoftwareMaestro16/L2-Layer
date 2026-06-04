import Image from "next/image";
import { TrendingUp } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { formatEnt, formatUsd } from "@/lib/format";
import type { AssetBalance } from "@/lib/types";

export function BalanceCard({ balance }: { balance: AssetBalance }) {
  return (
    <Card className="overflow-hidden">
      <CardContent className="grid gap-5 p-5 md:grid-cols-[1fr_auto] md:items-end">
        <div>
          <div className="mb-5 flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-lg border bg-muted">
              <Image src="/entropis.png" alt="Entropis" width={34} height={34} />
            </div>
            <div>
              <p className="text-sm font-semibold">{balance.name}</p>
              <p className="text-sm text-muted-foreground">Native mock gas asset</p>
            </div>
          </div>
          <p className="text-sm text-muted-foreground">Total balance</p>
          <div className="mt-1 flex flex-wrap items-baseline gap-2">
            <h2 className="text-4xl font-semibold">{formatEnt(balance.amount)}</h2>
            <span className="text-lg font-semibold text-muted-foreground">{balance.symbol}</span>
          </div>
          <p className="mt-2 text-sm text-muted-foreground">Mock valuation {formatUsd(balance.fiatValue)}</p>
        </div>
        <Badge variant="success" className="w-fit">
          <TrendingUp className="mr-1 h-3.5 w-3.5" />
          {balance.change24h.toFixed(1)}% today
        </Badge>
      </CardContent>
    </Card>
  );
}
