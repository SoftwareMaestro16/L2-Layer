import { ExplorerShell } from "@/components/explorer-shell";
import { AccountView } from "@/components/account-view";

export default async function AccountPage({ params }: { params: Promise<{ address: string }> }) {
  const { address } = await params;
  return (
    <ExplorerShell>
      <AccountView address={decodeURIComponent(address)} />
    </ExplorerShell>
  );
}
