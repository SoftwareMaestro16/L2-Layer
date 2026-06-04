import { Gem, Layers3 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatEnt, formatUsd } from "@/lib/format";
import type { Collectible, TokenHolding } from "@/lib/types";

export function AssetSections({
  tokens,
  collectibles
}: {
  tokens: TokenHolding[];
  collectibles: Collectible[];
}) {
  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Layers3 className="h-5 w-5 text-primary" />
            Tokens
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {tokens.map((token) => (
            <div key={token.id} className="flex items-center justify-between gap-3 rounded-lg border bg-muted/35 p-3">
              <div className="flex min-w-0 items-center gap-3">
                <div
                  className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-gradient-to-br ${token.color} text-xs font-bold text-white`}
                >
                  {token.symbol.slice(0, 2)}
                </div>
                <div className="min-w-0">
                  <p className="truncate text-sm font-semibold">{token.name}</p>
                  <p className="text-xs text-muted-foreground">{token.symbol}</p>
                </div>
              </div>
              <div className="text-right">
                <p className="text-sm font-semibold">
                  {formatEnt(token.amount)} {token.symbol}
                </p>
                <p className="text-xs text-muted-foreground">{formatUsd(token.fiatValue)}</p>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Gem className="h-5 w-5 text-primary" />
            Collectibles
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
          {collectibles.map((collectible) => (
            <div key={collectible.id} className="overflow-hidden rounded-lg border bg-card">
              <div className={`h-24 bg-gradient-to-br ${collectible.accent}`} />
              <div className="p-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="truncate text-sm font-semibold">{collectible.name}</p>
                    <p className="text-xs text-muted-foreground">{collectible.collection}</p>
                  </div>
                  <span className="rounded-md border border-violet-200 bg-violet-50 px-2 py-0.5 text-xs font-semibold text-violet-800 dark:border-violet-500/40 dark:bg-violet-500/15 dark:text-violet-100">
                    {collectible.rarity}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
