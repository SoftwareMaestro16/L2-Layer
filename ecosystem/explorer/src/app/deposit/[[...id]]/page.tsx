"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { ResourceLookup } from "@/components/resource-lookup";
import { getDeposit } from "@/lib/api";

export default function DepositPage() {
  const params = useParams<{ id?: string[] }>();
  return (
    <ExplorerShell>
      <ResourceLookup
        initialValue={params.id?.join("/") ?? ""}
        label="Deposit id"
        load={getDeposit}
        placeholder="64 hex deposit id"
        title="Deposit status"
      />
    </ExplorerShell>
  );
}
