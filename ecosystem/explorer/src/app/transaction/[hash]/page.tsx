"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { TransactionView } from "@/components/transaction-view";

export default function TransactionPage() {
  const params = useParams<{ hash: string }>();
  return (
    <ExplorerShell>
      <TransactionView hash={decodeURIComponent(params.hash)} />
    </ExplorerShell>
  );
}
