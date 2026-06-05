"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { ResourceLookup } from "@/components/resource-lookup";
import { getContractState } from "@/lib/api";

export default function ContractPage() {
  const params = useParams<{ id?: string[] }>();
  return (
    <ExplorerShell>
      <ResourceLookup
        initialValue={params.id?.join("/") ?? ""}
        label="Contract address"
        load={getContractState}
        placeholder="EX... or 8:..."
        title="Contract state"
      />
    </ExplorerShell>
  );
}
