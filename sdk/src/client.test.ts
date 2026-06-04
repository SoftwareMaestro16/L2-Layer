import assert from "node:assert/strict";
import test from "node:test";
import { EntropisApiError, EntropisClient } from "./index.js";
import { hash, vectorTransaction } from "./test_support.js";

test("Entropis client maps faucet requests and API errors safely", async () => {
  const previousFetch = globalThis.fetch;
  const accountId = hash(0xaa);
  const calls: Array<{ url: string; init?: RequestInit }> = [];

  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: input.toString(), init });
    if (input.toString().endsWith("/v1/admin/faucet/ent")) {
      assert.equal((init?.headers as Record<string, string>).authorization, "Bearer operator");
      assert.deepEqual(JSON.parse(init?.body as string), { account_id: accountId });
      return new Response(
        JSON.stringify({
          account_id: accountId,
          amount_ent: "1000",
          amount_base_units: "1000000000000",
          deposit_id: hash(0xdd),
          granted: true,
        }),
        { status: 200 },
      );
    }
    return new Response(JSON.stringify({ error: "nonce_locked" }), { status: 409 });
  }) as typeof fetch;

  try {
    const client = new EntropisClient("http://127.0.0.1:8080/", { adminToken: "operator" });
    const faucet = await client.requestEntFaucet(accountId);

    assert.equal(client.baseUrl, "http://127.0.0.1:8080");
    assert.equal(faucet.granted, true);
    assert.equal(calls[0].url, "http://127.0.0.1:8080/v1/admin/faucet/ent");
    await assert.rejects(
      client.submitTx(vectorTransaction()),
      (error) =>
        error instanceof EntropisApiError &&
        error.status === 409 &&
        error.publicMessage === "nonce_locked",
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});
