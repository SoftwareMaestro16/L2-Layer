"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { ResourceLookup } from "@/components/resource-lookup";
import { getWithdrawal } from "@/lib/api";

export default function WithdrawalPage() {
  const params = useParams<{ id?: string[] }>();
  return (
    <ExplorerShell>
      <ResourceLookup
        initialValue={params.id?.join("/") ?? ""}
        label="Withdrawal id"
        load={getWithdrawal}
        placeholder="64 hex withdrawal id"
        title="Withdrawal status"
      />
    </ExplorerShell>
  );
}
