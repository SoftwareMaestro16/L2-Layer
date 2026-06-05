import { describe, expect, it } from "vitest";
import { enwalletSendLink, formatBaseUnits, isProbablyHash, shortHash } from "./format";

describe("format helpers", () => {
  it("formats base units", () => {
    expect(formatBaseUnits("1000000000")).toBe("1");
    expect(formatBaseUnits("1234500000")).toBe("1.2345");
  });

  it("detects transaction hashes", () => {
    expect(isProbablyHash("a".repeat(64))).toBe(true);
    expect(isProbablyHash("EXjdAddress")).toBe(false);
  });

  it("shortens hashes", () => {
    expect(shortHash("abcdef0123456789", 4, 4)).toBe("abcd...6789");
  });

  it("builds EnWallet send links", () => {
    expect(enwalletSendLink("8:abc", 0, "10")).toContain("to=8%3Aabc");
    expect(enwalletSendLink("8:abc", 0, "10")).toContain("amount=10");
  });
});
