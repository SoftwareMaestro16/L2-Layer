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

export function ReceivePanel({ address }: { address: string }) {
  const [copied, setCopied] = useState(false);

  async function copyAddress() {
    await navigator.clipboard.writeText(address);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
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
          <DialogTitle>Receive ENT</DialogTitle>
          <DialogDescription>
            Mock receive address only. No TON bridge payload or deposit transaction is created here.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="mx-auto grid h-44 w-44 place-items-center rounded-lg border bg-muted text-center text-xs font-semibold text-muted-foreground">
            Mock QR
          </div>
          <div className="rounded-lg border bg-muted/50 p-3">
            <p className="break-all text-sm font-semibold">{address}</p>
          </div>
          <Button className="w-full" onClick={copyAddress}>
            <Copy className="h-4 w-4" />
            {copied ? "Copied address" : "Copy address"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
