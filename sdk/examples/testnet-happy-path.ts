import nacl from "tweetnacl";
import {
  EntropisClient,
  accountIdFromKeyPair,
  claimWithdrawalTonConnectMessage,
  depositTonTonConnectMessage,
  signTransferTransaction,
  signWithdrawTransaction,
  txHash,
  withdrawalId,
} from "@ton-l2-rollup/sdk";

const apiUrl = process.env.ENTROPIS_API_URL ?? "http://127.0.0.1:8080";
const chainId = process.env.ENTROPIS_CHAIN_ID ?? "entropis-testnet";
const keyPair = nacl.sign.keyPair();
const client = new EntropisClient(apiUrl);
const accountId = accountIdFromKeyPair(keyPair);

const operatorToken = process.env.ENTROPIS_ADMIN_TOKEN;
if (operatorToken) {
  const operator = new EntropisClient(apiUrl, { adminToken: operatorToken });
  await operator.requestEntFaucet(accountId);
}

const account = await client.getAccount(accountId);
const transfer = signTransferTransaction({
  chainId,
  from: accountId,
  nonce: account.nonce,
  to: process.env.ENTROPIS_DEMO_RECIPIENT_ID ?? accountId,
  assetId: 0,
  amount: "1000000000",
  gasLimit: 1000,
  maxGasPrice: "1",
  keyPair,
});

await client.submitTx(transfer);

const depositMessage = depositTonTonConnectMessage({
  vaultAddress: process.env.ENTROPIS_VAULT_ADDRESS ?? "<AssetVault testnet address>",
  queryId: Date.now(),
  amount: "100000000",
  l2Recipient: accountId,
});

console.log("Send this TON Connect deposit message:", depositMessage);

const refreshed = await client.getAccount(accountId);
const withdraw = signWithdrawTransaction({
  chainId,
  from: accountId,
  nonce: refreshed.nonce,
  assetId: 1,
  amount: "100000000",
  l1Recipient: process.env.ENTROPIS_L1_RECIPIENT ?? "<recipient TON testnet address>",
  gasLimit: 1000,
  maxGasPrice: "1",
  keyPair,
});
const withdrawHash = txHash(withdraw);
await client.submitTx(withdraw);

const withdrawalProof = await client.getWithdrawalProof(
  withdrawalId(
    withdrawHash,
    1,
    "100000000",
    accountId,
    process.env.ENTROPIS_L1_RECIPIENT ?? "<recipient TON testnet address>",
  ),
);
const claimMessage = claimWithdrawalTonConnectMessage({
  rollupRootAddress: process.env.ENTROPIS_ROLLUP_ROOT_ADDRESS ?? "<RollupRoot testnet address>",
  proof: withdrawalProof,
  amount: "150000000",
});

console.log("Send this TON Connect claim message:", claimMessage);

