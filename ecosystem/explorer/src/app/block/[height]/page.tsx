"use client";

import { useParams } from "next/navigation";
import { ExplorerShell } from "@/components/explorer-shell";
import { ResourceLookup } from "@/components/resource-lookup";
import { getBlock, getBlockFinality } from "@/lib/api";

export default function BlockPage() {
  const params = useParams<{ height: string }>();
  return (
    <ExplorerShell>
      <ResourceLookup
        initialValue={params.height}
        label="Block height"
        load={async (apiBase, value) => ({
          block: await getBlock(apiBase, value),
          finality: await getBlockFinality(apiBase, value).catch((error) => ({
            status: "unavailable",
            error: error instanceof Error ? error.message : "finality lookup failed",
          })),
        })}
        placeholder="0"
        title="Block state"
      />
    </ExplorerShell>
  );
}
