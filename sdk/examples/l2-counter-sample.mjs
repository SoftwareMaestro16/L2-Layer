import nacl from "tweetnacl";
import {
  EntropisClient,
  accountIdFromKeyPair,
  hashDomain,
  l2RawAddress,
  l2UserFriendlyAddress,
  sampleCounterIncrementBodyBase64,
  sampleCounterInitialState,
  signCallContractTransaction,
  signDeployContractTransaction,
} from "../dist/index.js";
import { EntropisAdminClient } from "../dist/admin.js";

const apiUrl = process.env.ENTROPIS_API_URL ?? "http://127.0.0.1:8080";
const chainId = process.env.ENTROPIS_CHAIN_ID ?? "entropis-testnet";
const adminToken = process.env.ENTROPIS_ADMIN_TOKEN;
const increment = process.env.ENTROPIS_COUNTER_INCREMENT ?? "1";
const keyPair = nacl.sign.keyPair();
const accountId = accountIdFromKeyPair(keyPair);
const contractId = hashDomain("l2.sample.counter.contract.v1", [
  Buffer.from(accountId, "hex"),
  Buffer.from(process.env.ENTROPIS_COUNTER_SALT ?? "public-demo", "utf8"),
]);
const client = new EntropisClient(apiUrl);
const admin = adminToken ? new EntropisAdminClient(apiUrl, { adminToken }) : null;

console.log("Throwaway test account raw:", l2RawAddress(accountId));
console.log("Throwaway test account friendly:", l2UserFriendlyAddress(accountId));
console.log("Sample counter contract raw:", l2RawAddress(contractId));
console.log("Sample counter contract friendly:", l2UserFriendlyAddress(contractId));

if (admin) {
  await admin.requestEntFaucet(accountId);
  await admin.produceBlock();
}

const account = await client.getAccount(accountId);
const initialState = sampleCounterInitialState(0);
const deploy = signDeployContractTransaction({
  chainId,
  from: accountId,
  nonce: account.nonce,
  contract: contractId,
  codeBocBase64: initialState.code_boc_base64,
  dataBocBase64: initialState.data_boc_base64,
  gasLimit: 50,
  maxGasPrice: "1",
  keyPair,
});
const deployResult = await client.submitTx(deploy);
console.log("Deploy tx:", deployResult.tx_hash);

if (admin) {
  await admin.produceBlock();
}

const afterDeploy = await client.getAccount(accountId);
const call = signCallContractTransaction({
  chainId,
  from: accountId,
  nonce: afterDeploy.nonce,
  contract: contractId,
  bodyBocBase64: sampleCounterIncrementBodyBase64(increment),
  gasLimit: 50,
  maxGasPrice: "1",
  keyPair,
});
const callResult = await client.submitTx(call);
console.log("Increment tx:", callResult.tx_hash);

if (admin) {
  await admin.produceBlock();
  console.log("Counter state:", await client.getSampleCounter(contractId));
  console.log("Get currentCounter:", await client.getContractMethod(contractId, "currentCounter"));
} else {
  console.log("Set ENTROPIS_ADMIN_TOKEN to auto-produce local blocks and read the counter.");
}
