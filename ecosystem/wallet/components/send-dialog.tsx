"use client";

import { FormEvent, ReactNode, useState } from "react";
import { Send } from "lucide-react";
import { ZodError } from "zod";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { sendTransferSchema } from "@/lib/schemas";
import { useWalletStore } from "@/store/wallet-store";

export function SendDialog({
  children,
  onSubmitted
}: {
  children: ReactNode;
  onSubmitted: () => Promise<void>;
}) {
  const sendTransfer = useWalletStore((state) => state.sendTransfer);
  const [open, setOpen] = useState(false);
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [memo, setMemo] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function reset() {
    setRecipient("");
    setAmount("");
    setMemo("");
    setError(null);
    setStatus(null);
    setSubmitting(false);
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setStatus(null);
    setSubmitting(true);

    try {
      const parsed = sendTransferSchema.parse({ recipient, amount, memo });
      const result = await sendTransfer(parsed);
      if (!result.ok) {
        setError(result.message);
        setSubmitting(false);
        return;
      }
      setStatus(`Submitted ${result.txHash.slice(0, 12)}...`);
      await onSubmitted();
      reset();
      setOpen(false);
    } catch (validationError) {
      setSubmitting(false);
      if (validationError instanceof ZodError) {
        setError(validationError.issues[0]?.message ?? "Invalid transfer.");
      } else {
        setError(validationError instanceof Error ? validationError.message : "Invalid transfer.");
      }
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen);
        if (!nextOpen) {
          reset();
        }
      }}
    >
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Send ENT</DialogTitle>
          <DialogDescription>
            Signs a canonical L2 transfer locally and submits it to the configured Entropis node.
          </DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={handleSubmit}>
          <div className="space-y-2">
            <Label htmlFor="recipient">Recipient</Label>
            <Input
              id="recipient"
              value={recipient}
              onChange={(event) => setRecipient(event.target.value)}
              placeholder="EX... or 8:..."
              spellCheck={false}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="amount">Amount</Label>
            <Input
              id="amount"
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
              placeholder="25.5"
              inputMode="decimal"
            />
            <p className="text-xs text-muted-foreground">
              Asset 0 ENT is sent. Gas is charged in ENT by the L2 executor.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="memo">Memo</Label>
            <Textarea
              id="memo"
              value={memo}
              onChange={(event) => setMemo(event.target.value)}
              placeholder="Local note, not sent on-chain"
            />
          </div>

          {error ? <p className="text-sm font-semibold text-destructive">{error}</p> : null}
          {status ? <p className="text-sm font-semibold text-muted-foreground">{status}</p> : null}

          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                Cancel
              </Button>
            </DialogClose>
            <Button type="submit" disabled={submitting}>
              <Send className="h-4 w-4" />
              {submitting ? "Submitting" : "Sign and send"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
