"use client";

import { useParams } from "next/navigation";
import { AccountView } from "@/components/account-view";
import { ExplorerShell } from "@/components/explorer-shell";

export default function AccountPage() {
  const params = useParams<{ address: string }>();
  return (
    <ExplorerShell>
      <AccountView address={decodeURIComponent(params.address)} />
    </ExplorerShell>
  );
}
