import { ExplorerShell } from "@/components/explorer-shell";
import { TransactionView } from "@/components/transaction-view";

export default async function TransactionPage({ params }: { params: Promise<{ hash: string }> }) {
  const { hash } = await params;
  return (
    <ExplorerShell>
      <TransactionView hash={decodeURIComponent(hash)} />
    </ExplorerShell>
  );
}
