import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { EntropisClient } from "./index.js";

const sdkRoot = dirname(dirname(fileURLToPath(import.meta.url)));

test("package exports keep browser and admin SDK boundaries separate", () => {
  const packageJson = JSON.parse(readSdkFile("package.json")) as {
    files?: string[];
    exports?: Record<string, { default?: string; types?: string }>;
    scripts?: Record<string, string>;
  };

  assert.deepEqual(packageJson.files, ["dist"]);
  assert.equal(packageJson.exports?.["."]?.default, "./dist/index.js");
  assert.equal(packageJson.exports?.["./browser"]?.default, "./dist/browser.js");
  assert.equal(packageJson.exports?.["./admin"]?.default, "./dist/admin.js");
  assert.notEqual(
    packageJson.exports?.["./browser"]?.default,
    packageJson.exports?.["./admin"]?.default,
  );
  assert.match(packageJson.scripts?.build ?? "", /^npm run clean && /);
});

test("browser entrypoint does not import admin helpers or admin token plumbing", () => {
  const browserSource = readSdkFile("src/browser.ts");

  assert.doesNotMatch(browserSource, /EntropisAdminClient/);
  assert.doesNotMatch(browserSource, /adminToken/);
  assert.doesNotMatch(browserSource, /\/v1\/admin\//);
  assert.doesNotMatch(browserSource, /from "\.\/admin\.js"/);
});

test("public SDK examples do not print secret material", () => {
  const examplesDir = join(sdkRoot, "examples");
  for (const fileName of readdirSync(examplesDir)) {
    if (!/\.(mjs|ts)$/.test(fileName)) {
      continue;
    }
    const lines = readFileSync(join(examplesDir, fileName), "utf8").split(/\r?\n/);
    lines.forEach((line, index) => {
      if (!/\bconsole\.(log|warn|error)\b/.test(line)) {
        return;
      }
      assert.doesNotMatch(
        line,
        /\b(secretKey|privateKey|mnemonic|recoveryWords|adminToken|seed)\b/i,
        `${fileName}:${index + 1} prints secret-like material`,
      );
    });
  }
});

test("receipt polling timeout stays bounded and reports only public API error text", async () => {
  const previousFetch = globalThis.fetch;
  const txHash = "ee".repeat(32);
  globalThis.fetch = (async () =>
    new Response(JSON.stringify({ error: "transaction not found" }), {
      status: 404,
      statusText: "Not Found",
    })) as typeof fetch;

  try {
    const client = new EntropisClient("http://127.0.0.1:8080/");
    await assert.rejects(
      client.waitForTxReceipt(txHash, { intervalMs: 1, timeoutMs: 2 }),
      (error) =>
        error instanceof Error &&
        error.message.includes(`timed out waiting for tx ${txHash}`) &&
        error.message.includes("last API error: transaction not found") &&
        !/secret|token|private/i.test(error.message),
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

function readSdkFile(relativePath: string): string {
  return readFileSync(join(sdkRoot, relativePath), "utf8");
}
