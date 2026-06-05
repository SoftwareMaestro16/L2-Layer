import { describe, expect, it } from "vitest";
import { formatAmount, isProbablyHash, shortHash } from "@/lib/format";

describe("format helpers", () => {
  it("shortens hashes", () => {
    expect(shortHash("a".repeat(64))).toBe("aaaaaaaaaa...aaaaaaaa");
  });

  it("formats integer amounts", () => {
    expect(formatAmount("1000000")).toBe("1,000,000");
  });

  it("detects hex hashes", () => {
    expect(isProbablyHash("0x" + "f".repeat(64))).toBe(true);
    expect(isProbablyHash("not-a-hash")).toBe(false);
  });
});
