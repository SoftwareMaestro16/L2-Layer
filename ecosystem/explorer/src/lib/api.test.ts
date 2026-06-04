import { describe, expect, it } from "vitest";
import { accountTransactionsPath, normalizeApiBase } from "@/lib/api";

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
});
