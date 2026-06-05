import type { FaucetConfig } from "./config.js";
import type { EntropisNodeClient } from "./node-client.js";
import type { FaucetStore } from "./store.js";

export class FaucetBatchWorker {
  private timer: NodeJS.Timeout | null = null;
  private running = false;

  constructor(
    private readonly config: FaucetConfig,
    private readonly store: FaucetStore,
    private readonly nodeClient: EntropisNodeClient,
  ) {}

  start(): void {
    if (this.timer) {
      return;
    }
    this.timer = setInterval(() => {
      void this.drainOnce();
    }, this.config.batchIntervalMs);
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  async drainOnce(): Promise<void> {
    if (this.running) {
      return;
    }
    const claims = this.store.takePending(this.config.maxBatchSize);
    if (claims.length === 0) {
      return;
    }

    this.running = true;
    const batch = this.store.startBatch(claims);
    try {
      const results = await this.nodeClient.submitClaims(
        claims.map((claim) => ({
          claimId: claim.claimId,
          accountId: claim.accountId,
        })),
      );
      this.store.completeBatch(batch.batchId, results);
    } catch (error) {
      this.store.failBatch(batch.batchId, safeReason(error));
    } finally {
      this.running = false;
    }
  }
}

function safeReason(error: unknown): string {
  if (error instanceof Error && /^[a-z0-9_]+$/u.test(error.message)) {
    return error.message;
  }
  return "faucet_batch_failed";
}
