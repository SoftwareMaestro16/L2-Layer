import { describe, expect, it } from "vitest";
import {
  accountTransactionsPath,
  getAccount,
  getBlocks,
  getExplorerSummary,
  normalizeApiBase,
} from "@/lib/api";
import { operatorResources } from "@/lib/operator-proxy";

describe("api helpers", () => {
  it("normalizes api base", () => {
    expect(normalizeApiBase("http://127.0.0.1:8080///")).toBe(
      "http://127.0.0.1:8080",
    );
  });

  it("builds account transaction cursor paths", () => {
    expect(
      accountTransactionsPath("8:abc", {
        before_height: 7,
        before_index: 2,
      }),
    ).toBe(
      "/v1/explorer/account/8%3Aabc/transactions?limit=25&before_height=7&before_index=2",
    );
  });

  it("fetches and parses explorer summary", async () => {
    const previousFetch = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response(
        JSON.stringify({
          latest_block: null,
          latest_batch_commit: null,
          latest_confirmed_commit: null,
          latest_finalization: null,
          latest_finalized_batch: null,
        }),
        { status: 200 },
      )) as typeof fetch;

    try {
      await expect(getExplorerSummary("http://node.local/")).resolves.toEqual({
        latest_block: null,
        latest_batch_commit: null,
        latest_confirmed_commit: null,
        latest_finalization: null,
        latest_finalized_batch: null,
      });
    } finally {
      globalThis.fetch = previousFetch;
    }
  });

  it("builds latest block list requests", async () => {
    const calls: string[] = [];
    const previousFetch = globalThis.fetch;
    globalThis.fetch = (async (url: string | URL | Request) => {
      calls.push(url.toString());
      return new Response(JSON.stringify({ items: [], next_before_height: null }), {
        status: 200,
      });
    }) as typeof fetch;

    try {
      await getBlocks("http://node.local///", 9, 3);
      expect(calls[0]).toBe("http://node.local/v1/explorer/blocks?limit=3&before_height=9");
    } finally {
      globalThis.fetch = previousFetch;
    }
  });

  it("hides raw upstream error text from public clients", async () => {
    const previousFetch = globalThis.fetch;
    globalThis.fetch = (async () =>
      new Response("stack trace with provider internals", { status: 502 })) as typeof fetch;

    try {
      await expect(getAccount("http://node.local", "0:abc")).rejects.toMatchObject({
        message: "request failed",
        status: 502,
      });
    } finally {
      globalThis.fetch = previousFetch;
    }
  });

  it("exposes operator proxy resources without browser admin tokens", () => {
    expect(Object.keys(operatorResources)).toEqual([
      "readiness",
      "metrics",
      "failures",
      "relayer",
      "finalizer",
      "signer",
      "faucet",
    ]);
  });
});
