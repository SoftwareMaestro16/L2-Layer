"use client";

import { ExplorerShell } from "@/components/explorer-shell";
import { PublicDashboard } from "@/components/public-dashboard";

export default function Home() {
  return (
    <ExplorerShell>
      <PublicDashboard />
    </ExplorerShell>
  );
}
