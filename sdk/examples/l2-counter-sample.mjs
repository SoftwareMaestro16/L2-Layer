import nacl from "tweetnacl";
import {
  EntropisClient,
  accountIdFromKeyPair,
  hashDomain,
  sampleCounterIncrementBodyBase64,
  sampleCounterInitialState,
  signCallContractTransaction,
  signDeployContractTransaction,
} from "../dist/index.js";

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
const client = new EntropisClient(apiUrl, adminToken ? { adminToken } : {});

console.log("Throwaway test account:", accountId);
console.log("Sample counter contract:", contractId);

if (adminToken) {
  await client.requestEntFaucet(accountId);
  await client.adminProduceBlock();
}

const account = await client.getAccount(accountId);
const initialState = sampleCounterInitialState(0);
const deploy = signDeployContractTransaction({
  chainId,
  from: accountId,
  nonce: account.nonce,
  contract: contractId,
  codeHash: initialState.code_hash,
  dataHash: initialState.data_hash,
  storageRoot: initialState.storage_root,
  gasLimit: 50,
  maxGasPrice: "1",
  keyPair,
});
const deployResult = await client.submitTx(deploy);
console.log("Deploy tx:", deployResult.tx_hash);

if (adminToken) {
  await client.adminProduceBlock();
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

if (adminToken) {
  await client.adminProduceBlock();
  console.log("Counter state:", await client.getSampleCounter(contractId));
} else {
  console.log("Set ENTROPIS_ADMIN_TOKEN to auto-produce local blocks and read the counter.");
}
