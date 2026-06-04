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

export function SendDialog({ children }: { children: ReactNode }) {
  const sendMockTransfer = useWalletStore((state) => state.sendMockTransfer);
  const [open, setOpen] = useState(false);
  const [recipient, setRecipient] = useState("");
  const [amount, setAmount] = useState("");
  const [memo, setMemo] = useState("");
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setRecipient("");
    setAmount("");
    setMemo("");
    setError(null);
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    try {
      const parsed = sendTransferSchema.parse({ recipient, amount, memo });
      const result = sendMockTransfer(parsed);
      if (!result.ok) {
        setError(result.message);
        return;
      }
      reset();
      setOpen(false);
    } catch (validationError) {
      if (validationError instanceof ZodError) {
        setError(validationError.issues[0]?.message ?? "Invalid transfer.");
      } else {
        setError("Invalid transfer.");
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
            Creates a pending mock transaction in local UI state. No signature or network request is made.
          </DialogDescription>
        </DialogHeader>

        <form className="space-y-4" onSubmit={handleSubmit}>
          <div className="space-y-2">
            <Label htmlFor="recipient">Recipient</Label>
            <Input
              id="recipient"
              value={recipient}
              onChange={(event) => setRecipient(event.target.value)}
              placeholder="EX..."
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
            <p className="text-xs text-muted-foreground">Mock fee: 0.013 ENT.</p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="memo">Memo</Label>
            <Textarea
              id="memo"
              value={memo}
              onChange={(event) => setMemo(event.target.value)}
              placeholder="Optional note"
            />
          </div>

          {error ? <p className="text-sm font-semibold text-destructive">{error}</p> : null}

          <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                Cancel
              </Button>
            </DialogClose>
            <Button type="submit">
              <Send className="h-4 w-4" />
              Create mock send
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
