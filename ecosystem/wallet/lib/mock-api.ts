import { mockNetworkSnapshot } from "@/lib/mock-data";
import type { NetworkSnapshot } from "@/lib/types";

const wait = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export async function fetchMockNetworkSnapshot(): Promise<NetworkSnapshot> {
  await wait(240);
  return {
    ...mockNetworkSnapshot,
    latestBatch: mockNetworkSnapshot.latestBatch + Math.floor(Date.now() / 60_000) % 12
  };
}
