"use client";

import { FormEvent, ReactNode, useState } from "react";
import { AlertTriangle, Send, ShieldCheck } from "lucide-react";
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
import type { TransferReview } from "@/lib/types";
import { useWalletStore } from "@/store/wallet-store";

export function SendDialog({
  children,
  onSubmitted
}: {
  children: ReactNode;
  onSubmitted: () => Promise<void>;
}) {
  const prepareTransfer = useWalletStore((state) => state.prepareTransfer);
  const submitReviewedTransfer = useWalletStore((state) => state.submitReviewedTransfer);
  const [open, setOpen] = useState(false);
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [memo, setMemo] = useState("");
  const [review, setReview] = useState<TransferReview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function reset() {
    setRecipient("");
    setAmount("");
    setMemo("");
    setReview(null);
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
      if (!review) {
        const prepared = await prepareTransfer(parsed);
        if (!prepared.ok) {
          setError(prepared.message);
        } else {
          setReview(prepared.review);
        }
        setSubmitting(false);
        return;
      }

      const result = await submitReviewedTransfer(review);
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
              onChange={(event) => {
                setRecipient(event.target.value);
                setReview(null);
              }}
              placeholder="EX... or 8:..."
              spellCheck={false}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="amount">Amount</Label>
            <Input
              id="amount"
              value={amount}
              onChange={(event) => {
                setAmount(event.target.value);
                setReview(null);
              }}
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
              onChange={(event) => {
                setMemo(event.target.value);
                setReview(null);
              }}
              placeholder="Local note, not sent on-chain"
            />
          </div>

          {review ? (
            <div className="space-y-3 rounded-lg border bg-muted/40 p-4 text-sm">
              <div className="flex items-center gap-2 font-semibold">
                <ShieldCheck className="h-4 w-4 text-primary" />
                Review before submitting
              </div>
              <ReviewRow label="Recipient" value={review.recipient} />
              <ReviewRow label="Amount" value={`${review.amount} ${review.symbol}`} />
              <ReviewRow label="Nonce" value={String(review.nonce)} />
              <ReviewRow label="Valid until block" value={String(review.validUntilBlock)} />
              <ReviewRow label="Gas limit" value={String(review.gasLimit)} />
              <ReviewRow label="Max gas price" value={review.maxGasPrice} />
              <ReviewRow label="Fee asset" value={String(review.feeAssetId)} />
              <ReviewRow label="Tx hash" value={`${review.txHash.slice(0, 16)}...${review.txHash.slice(-8)}`} />
              <p className="flex gap-2 text-xs text-muted-foreground">
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                Unknown contract calls are not part of this transfer flow; review contract payloads separately before
                signing.
              </p>
            </div>
          ) : null}

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
              {submitting ? "Submitting" : review ? "Confirm and submit" : "Review transfer"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ReviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 sm:grid-cols-[9rem_1fr]">
      <span className="text-muted-foreground">{label}</span>
      <strong className="break-all font-mono text-xs sm:text-sm">{value}</strong>
    </div>
  );
}
