"use client";

import { Copy, QrCode } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from "@/components/ui/dialog";
import type { WalletAccount } from "@/lib/types";

export function ReceivePanel({ account }: { account: WalletAccount }) {
  const [copied, setCopied] = useState<"friendly" | "raw" | null>(null);

  async function copyAddress(kind: "friendly" | "raw") {
    await navigator.clipboard.writeText(kind === "friendly" ? account.address : account.rawAddress);
    setCopied(kind);
    window.setTimeout(() => setCopied(null), 1200);
  }

  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline" className="h-12 w-full">
          <QrCode className="h-4 w-4" />
          Receive
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Receive on Entropis L2</DialogTitle>
          <DialogDescription>
            Use the EX address for wallet users. Use the raw address for scripts, API calls, and bridge payloads.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="mx-auto grid h-44 w-44 place-items-center rounded-lg border bg-gradient-to-br from-cyan-50 to-violet-100 text-center text-xs font-semibold text-muted-foreground dark:from-cyan-500/15 dark:to-violet-500/20">
            EX
          </div>
          <AddressBlock
            label="User-friendly"
            value={account.address}
            copied={copied === "friendly"}
            onCopy={() => copyAddress("friendly")}
          />
          <AddressBlock
            label="Raw"
            value={account.rawAddress}
            copied={copied === "raw"}
            onCopy={() => copyAddress("raw")}
          />
        </div>
      </DialogContent>
    </Dialog>
  );
}

function AddressBlock({
  label,
  value,
  copied,
  onCopy
}: {
  label: string;
  value: string;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs font-semibold uppercase text-muted-foreground">{label}</p>
      <div className="rounded-lg border bg-muted/50 p-3">
        <p className="break-all text-sm font-semibold">{value}</p>
      </div>
      <Button className="w-full" variant="outline" onClick={onCopy}>
        <Copy className="h-4 w-4" />
        {copied ? "Copied" : `Copy ${label.toLowerCase()}`}
      </Button>
    </div>
  );
}
