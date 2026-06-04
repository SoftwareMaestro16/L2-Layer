#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import nacl from "tweetnacl";
import {
  accountIdFromKeyPair,
  buildTransferTransaction,
  buildWithdrawTransaction,
  claimWithdrawalTonConnectMessage,
  depositTonTonConnectMessage,
  EntropisApiError,
  EntropisClient,
  L2_NATIVE_GAS_ASSET,
  signTransaction,
  txHash,
} from "../dist/index.js";

const DEFAULT_API_BASE_URL = "http://127.0.0.1:8080";
const TESTNET_WARNING =
  "TESTNET ONLY: this demo refuses non-testnet registries and is not for mainnet funds.";

main().catch((error) => {
  const safe = sanitizeError(error);
  console.error(JSON.stringify({ ok: false, error: safe }, null, 2));
  process.exitCode = 1;
});

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  if (!command || command === "help" || command === "--help") {
    printHelp();
    return;
  }

  const args = parseArgs(rest);
  const registry = loadRegistry(args);
  const client = new EntropisClient(apiBaseUrl(args), {
    adminToken: process.env.ENTROPIS_ADMIN_TOKEN,
  });

  switch (command) {
    case "generate-account":
      return generateAccount(args, registry);
    case "faucet":
      return requestFaucet(args, registry, client);
    case "transfer":
      return submitTransfer(args, registry, client);
    case "withdraw":
      return submitWithdraw(args, registry, client);
    case "deposit-payload":
      return buildDepositPayload(args, registry);
    case "get-proof":
      return getProof(args, registry, client);
    case "claim-withdrawal":
      return buildClaimPayload(args, registry, client);
    default:
      throw new Error(`unknown command: ${command}`);
  }
}

function printHelp() {
  console.log(`Entropis public testnet demo CLI

${TESTNET_WARNING}

Environment:
  ENTROPIS_API_BASE_URL       L2 API base URL, default ${DEFAULT_API_BASE_URL}
  ENTROPIS_REGISTRY_PATH      Testnet registry path, default deployments/testnet/entropis.json
  ENTROPIS_SECRET_KEY_HEX     Throwaway test key seed(32-byte) or secret key(64-byte) for signing
  ENTROPIS_ADMIN_TOKEN        Required only for admin-only faucet/dev flows

Commands:
  generate-account [--show-secret]
  faucet --account-id <hash32>
  transfer --to <hash32> --amount <base-units> [--nonce <n>] [--dry-run]
  withdraw --l1-recipient <ton-address> --amount <base-units> [--nonce <n>] [--dry-run]
  deposit-payload --l2-recipient <hash32> --amount <nanotons> [--query-id <n>]
  get-proof --withdrawal-id <hash32>
  claim-withdrawal --withdrawal-id <hash32> [--amount <nanotons>]
`);
}

function generateAccount(args, registry) {
  const keyPair = nacl.sign.keyPair();
  const out = baseOutput(registry, {
    account_id: accountIdFromKeyPair(keyPair),
    public_key: Buffer.from(keyPair.publicKey).toString("hex"),
  });
  if (args.showSecret) {
    out.secret_warning =
      "Throwaway test key only. Do not reuse this key and do not paste it into public logs.";
    out.secret_key_hex = Buffer.from(keyPair.secretKey).toString("hex");
  }
  printJson(out);
}

async function requestFaucet(args, registry, client) {
  requireAdminToken();
  const accountId = required(args, "account-id");
  const faucet = await client.requestEntFaucet(accountId);
  printJson(
    baseOutput(registry, {
      account_id: faucet.account_id,
      amount_ent: faucet.amount_ent,
      amount_base_units: faucet.amount_base_units,
      deposit_id: faucet.deposit_id,
      granted: faucet.granted,
    }),
  );
}

async function submitTransfer(args, registry, client) {
  const keyPair = keyPairFromEnv();
  const from = accountIdFromKeyPair(keyPair);
  const nonce = await resolveNonce(args, client, from);
  const unsigned = buildTransferTransaction({
    chainId: registry.chainId,
    from,
    nonce,
    to: required(args, "to"),
    assetId: args.assetId ?? L2_NATIVE_GAS_ASSET,
    amount: required(args, "amount"),
    gasLimit: args.gasLimit ?? "500",
    maxGasPrice: args.maxGasPrice ?? "42",
  });
  const signed = signTransaction(unsigned, keyPair);
  const localHash = txHash(signed);

  if (args.dryRun) {
    printJson(baseOutput(registry, { account_id: from, nonce, tx_hash: localHash, tx: signed }));
    return;
  }

  const response = await client.submitTx(signed);
  printJson(baseOutput(registry, { account_id: from, nonce, tx_hash: response.tx_hash }));
}

async function submitWithdraw(args, registry, client) {
  const keyPair = keyPairFromEnv();
  const from = accountIdFromKeyPair(keyPair);
  const nonce = await resolveNonce(args, client, from);
  const unsigned = buildWithdrawTransaction({
    chainId: registry.chainId,
    from,
    nonce,
    assetId: args.assetId ?? registry.tonAssetId ?? L2_NATIVE_GAS_ASSET,
    amount: required(args, "amount"),
    l1Recipient: required(args, "l1-recipient"),
    gasLimit: args.gasLimit ?? "500",
    maxGasPrice: args.maxGasPrice ?? "42",
  });
  const signed = signTransaction(unsigned, keyPair);
  const localHash = txHash(signed);

  if (args.dryRun) {
    printJson(baseOutput(registry, { account_id: from, nonce, tx_hash: localHash, tx: signed }));
    return;
  }

  const response = await client.submitTx(signed);
  printJson(baseOutput(registry, { account_id: from, nonce, tx_hash: response.tx_hash }));
}

function buildDepositPayload(args, registry) {
  const vaultAddress = args.vaultAddress ?? registry.assetVaultAddress;
  if (!vaultAddress) {
    throw new Error("AssetVault address is missing; pass --vault-address after testnet deploy");
  }
  const message = depositTonTonConnectMessage({
    vaultAddress,
    queryId: args.queryId ?? Date.now().toString(),
    amount: required(args, "amount"),
    l2Recipient: required(args, "l2-recipient"),
  });
  printJson(
    baseOutput(registry, {
      dry_run: true,
      target_contract: "AssetVault",
      tonconnect_message: message,
    }),
  );
}

async function getProof(args, registry, client) {
  const withdrawalId = required(args, "withdrawal-id");
  const proof = await client.getWithdrawalProof(withdrawalId);
  printJson(
    baseOutput(registry, {
      proof_id: proof.leaf.withdrawal_id,
      withdrawal_id: proof.leaf.withdrawal_id,
      block_height: proof.block_height,
      withdrawal_root: proof.withdrawal_root,
      leaf_index: proof.proof.leaf_index,
      proof,
    }),
  );
}

async function buildClaimPayload(args, registry, client) {
  const rollupRootAddress = args.rollupRootAddress ?? registry.rollupRootAddress;
  if (!rollupRootAddress) {
    throw new Error("RollupRoot address is missing; pass --rollup-root-address after testnet deploy");
  }
  const withdrawalId = required(args, "withdrawal-id");
  const proof = await client.getWithdrawalProof(withdrawalId);
  const message = claimWithdrawalTonConnectMessage({
    rollupRootAddress,
    proof,
    amount: args.amount ?? "150000000",
  });
  printJson(
    baseOutput(registry, {
      dry_run: true,
      target_contract: "RollupRoot",
      proof_id: proof.leaf.withdrawal_id,
      withdrawal_id: proof.leaf.withdrawal_id,
      block_height: proof.block_height,
      tonconnect_message: message,
    }),
  );
}

async function resolveNonce(args, client, accountId) {
  if (args.nonce !== undefined) {
    return args.nonce;
  }
  const account = await client.getAccount(accountId);
  return account.nonce;
}

function keyPairFromEnv() {
  const secret = process.env.ENTROPIS_SECRET_KEY_HEX;
  if (!secret) {
    throw new Error("ENTROPIS_SECRET_KEY_HEX is required for signing commands");
  }
  const cleaned = secret.startsWith("0x") ? secret.slice(2) : secret;
  if (!/^[0-9a-fA-F]+$/.test(cleaned)) {
    throw new Error("ENTROPIS_SECRET_KEY_HEX must be hex");
  }
  const bytes = Buffer.from(cleaned, "hex");
  if (bytes.length === 32) {
    return nacl.sign.keyPair.fromSeed(bytes);
  }
  if (bytes.length === 64) {
    return nacl.sign.keyPair.fromSecretKey(bytes);
  }
  throw new Error("ENTROPIS_SECRET_KEY_HEX must be 32-byte seed or 64-byte secret key");
}

function apiBaseUrl(args) {
  return args.apiBaseUrl ?? process.env.ENTROPIS_API_BASE_URL ?? DEFAULT_API_BASE_URL;
}

function loadRegistry(args) {
  const registryPath = resolveRegistryPath(args.registry ?? process.env.ENTROPIS_REGISTRY_PATH);
  const raw = JSON.parse(readFileSync(registryPath, "utf8"));
  if (raw.tonNetwork !== "testnet" || raw.chainId !== "entropis-testnet") {
    throw new Error("refusing non-testnet registry");
  }
  const active =
    raw.deployments.find((deployment) => deployment.id === raw.activeDeploymentId) ??
    raw.deployments[0];
  return {
    warning: TESTNET_WARNING,
    path: registryPath,
    chainId: raw.chainId,
    tonNetwork: raw.tonNetwork,
    activeDeploymentId: raw.activeDeploymentId,
    deploymentStatus: active?.status,
    rollupRootAddress: active?.contracts?.RollupRoot?.address ?? null,
    assetVaultAddress: active?.contracts?.AssetVault?.address ?? null,
    tonAssetId: active?.parameters?.tonAssetId,
  };
}

function resolveRegistryPath(candidate) {
  const candidates = candidate
    ? [candidate]
    : [
        path.join(process.cwd(), "deployments/testnet/entropis.json"),
        path.join(process.cwd(), "../deployments/testnet/entropis.json"),
      ];
  for (const item of candidates) {
    const resolved = path.resolve(item);
    if (existsSync(resolved)) {
      return resolved;
    }
  }
  throw new Error("testnet registry not found; set ENTROPIS_REGISTRY_PATH");
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) {
      throw new Error(`unexpected positional argument: ${token}`);
    }
    const key = toCamel(token.slice(2));
    if (key === "dryRun" || key === "showSecret") {
      out[key] = true;
      continue;
    }
    const value = argv[i + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`missing value for ${token}`);
    }
    out[key] = value;
    i += 1;
  }
  return out;
}

function toCamel(value) {
  return value.replace(/-([a-z])/g, (_, char) => char.toUpperCase());
}

function required(args, name) {
  const value = args[toCamel(name)];
  if (value === undefined || value === "") {
    throw new Error(`--${name} is required`);
  }
  return value;
}

function requireAdminToken() {
  if (!process.env.ENTROPIS_ADMIN_TOKEN) {
    throw new Error("ENTROPIS_ADMIN_TOKEN is required for the admin-only faucet endpoint");
  }
}

function baseOutput(registry, extra) {
  return {
    ok: true,
    warning: registry.warning,
    chain_id: registry.chainId,
    ton_network: registry.tonNetwork,
    registry_path: registry.path,
    active_deployment_id: registry.activeDeploymentId,
    deployment_status: registry.deploymentStatus,
    ...extra,
  };
}

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

function sanitizeError(error) {
  if (error instanceof EntropisApiError) {
    return {
      type: "api",
      status: error.status,
      message: error.publicMessage,
    };
  }
  const message = error instanceof Error ? error.message : String(error);
  return message.replace(process.env.ENTROPIS_ADMIN_TOKEN ?? "__NO_TOKEN__", "[redacted]");
}
