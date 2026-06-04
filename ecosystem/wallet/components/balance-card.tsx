import Image from "next/image";
import { Card, CardContent } from "@/components/ui/card";
import { formatEnt } from "@/lib/format";
import type { AssetBalance } from "@/lib/types";

export function BalanceCard({ balance }: { balance: AssetBalance }) {
  return (
    <Card className="overflow-hidden border-violet-100/80 bg-card/95 dark:border-violet-500/20">
      <CardContent className="grid gap-5 p-5 md:grid-cols-[1fr_auto] md:items-end">
        <div>
          <div className="mb-5 flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-lg border bg-gradient-to-br from-blue-50 to-violet-100 dark:from-blue-500/15 dark:to-violet-500/20">
              <Image src="/entropis.png" alt="Entropis" width={34} height={34} />
            </div>
            <div>
              <p className="text-sm font-semibold">{balance.name}</p>
              <p className="text-sm text-muted-foreground">Native L2 gas asset</p>
            </div>
          </div>
          <p className="text-sm text-muted-foreground">Total balance</p>
          <div className="mt-1 flex flex-wrap items-baseline gap-2">
            <h2 className="text-4xl font-semibold">{formatEnt(balance.amount)}</h2>
            <span className="text-lg font-semibold text-muted-foreground">{balance.symbol}</span>
          </div>
          <p className="mt-2 text-sm text-muted-foreground">
            {balance.baseUnits} base units, {balance.decimals} decimals
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
