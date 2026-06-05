import { execFileSync } from "node:child_process";
import path from "node:path";
import nacl from "tweetnacl";
import {
  EntropisClient,
  accountIdFromKeyPair,
  contractCellHash,
  hashDomain,
  l2RawAddress,
  l2UserFriendlyAddress,
  signCallContractTransaction,
  signDeployContractTransaction,
} from "../dist/index.js";
import { EntropisAdminClient } from "../dist/admin.js";

const apiUrl = process.env.ENTROPIS_API_URL ?? "http://127.0.0.1:8080";
const chainId = process.env.ENTROPIS_CHAIN_ID ?? "entropis-testnet";
const adminToken = process.env.ENTROPIS_ADMIN_TOKEN;
const sourcePath = process.env.ENTROPIS_TOLK_SOURCE;
const dataBocBase64 = process.env.ENTROPIS_DATA_BOC_BASE64;
const callBodyBocBase64 = process.env.ENTROPIS_CALL_BODY_BOC_BASE64;
const getMethod = process.env.ENTROPIS_GET_METHOD;

if (!sourcePath && !process.env.ENTROPIS_CODE_BOC_BASE64) {
  throw new Error("set ENTROPIS_TOLK_SOURCE or ENTROPIS_CODE_BOC_BASE64");
}
if (!dataBocBase64) {
  throw new Error("set ENTROPIS_DATA_BOC_BASE64 for the contract initial data cell");
}

const keyPair = nacl.sign.keyPair();
const accountId = accountIdFromKeyPair(keyPair);
const codeBocBase64 = process.env.ENTROPIS_CODE_BOC_BASE64 ?? compileTolk(sourcePath);
const contractId = hashDomain("l2.contract.deploy.v1", [
  Buffer.from(accountId, "hex"),
  Buffer.from(contractCellHash(codeBocBase64), "hex"),
  Buffer.from(process.env.ENTROPIS_CONTRACT_SALT ?? "acton-demo", "utf8"),
]);
const client = new EntropisClient(apiUrl);
const admin = adminToken ? new EntropisAdminClient(apiUrl, { adminToken }) : null;

console.log("Throwaway test account raw:", l2RawAddress(accountId));
console.log("Throwaway test account friendly:", l2UserFriendlyAddress(accountId));
console.log("Contract raw:", l2RawAddress(contractId));
console.log("Contract friendly:", l2UserFriendlyAddress(contractId));
console.log("Code cell hash:", contractCellHash(codeBocBase64));
console.log("Data cell hash:", contractCellHash(dataBocBase64));

if (admin) {
  await admin.requestEntFaucet(accountId);
  await admin.produceBlock();
}

const account = await client.getAccount(accountId);
const deploy = signDeployContractTransaction({
  chainId,
  from: accountId,
  nonce: account.nonce,
  contract: contractId,
  codeBocBase64,
  dataBocBase64,
  gasLimit: process.env.ENTROPIS_DEPLOY_GAS_LIMIT ?? "1000",
  maxGasPrice: process.env.ENTROPIS_MAX_GAS_PRICE ?? "1",
  keyPair,
});
console.log("Deploy tx:", (await client.submitTx(deploy)).tx_hash);

if (admin) {
  await admin.produceBlock();
}

if (callBodyBocBase64) {
  const afterDeploy = await client.getAccount(accountId);
  const call = signCallContractTransaction({
    chainId,
    from: accountId,
    nonce: afterDeploy.nonce,
    contract: contractId,
    bodyBocBase64: callBodyBocBase64,
    gasLimit: process.env.ENTROPIS_CALL_GAS_LIMIT ?? "1000",
    maxGasPrice: process.env.ENTROPIS_MAX_GAS_PRICE ?? "1",
    keyPair,
  });
  console.log("Call tx:", (await client.submitTx(call)).tx_hash);
  if (admin) {
    await admin.produceBlock();
  }
}

if (getMethod) {
  console.log("Get method:", await client.getContractMethod(contractId, getMethod));
}

function compileTolk(source) {
  const resolved = path.resolve(process.cwd(), source);
  return execFileSync("acton", ["compile", "--base64-only", resolved], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  }).trim();
}
