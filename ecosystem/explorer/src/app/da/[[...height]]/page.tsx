"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { ResourceLookup } from "@/components/resource-lookup";
import { getDaPayload } from "@/lib/api";

export default function DaPage() {
  const params = useParams<{ height?: string[] }>();
  return (
    <ExplorerShell>
      <ResourceLookup
        initialValue={params.height?.join("/") ?? ""}
        label="Block height"
        load={getDaPayload}
        placeholder="0"
        title="DA payload"
      />
    </ExplorerShell>
  );
}
