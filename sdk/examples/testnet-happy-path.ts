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
} from "../src/index.ts";

const apiUrl = process.env.ENTROPIS_API_URL ?? "http://127.0.0.1:8080";
const chainId = process.env.ENTROPIS_CHAIN_ID ?? "entropis-testnet";
const keyPair = nacl.sign.keyPair();
const client = new EntropisClient(apiUrl);
const accountId = accountIdFromKeyPair(keyPair);

const operatorToken = process.env.ENTROPIS_ADMIN_TOKEN;
if (operatorToken) {
  const operator = new EntropisClient(apiUrl, { adminToken: operatorToken });
  const faucet = await operator.requestEntFaucet(accountId);
  console.log("ENT faucet:", faucet);
  const faucetBlock = await operator.adminProduceBlock();
  console.log("Produced faucet block:", faucetBlock ?? "no pending deposits");
}

const account = await client.getAccount(accountId);
console.log("Account after faucet:", account);
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

const transferResult = await client.submitTx(transfer);
console.log("Submitted transfer:", transferResult);

if (operatorToken) {
  const operator = new EntropisClient(apiUrl, { adminToken: operatorToken });
  const transferBlock = await operator.adminProduceBlock();
  console.log("Produced transfer block:", transferBlock ?? "no pending transactions");
}

if (process.env.ENTROPIS_VAULT_ADDRESS) {
  const depositMessage = depositTonTonConnectMessage({
    vaultAddress: process.env.ENTROPIS_VAULT_ADDRESS,
    queryId: Date.now(),
    amount: "100000000",
    l2Recipient: accountId,
  });

  console.log("Send this TON Connect deposit message:", depositMessage);
}

if (
  process.env.ENTROPIS_RUN_WITHDRAWAL === "1" &&
  process.env.ENTROPIS_L1_RECIPIENT &&
  process.env.ENTROPIS_ROLLUP_ROOT_ADDRESS
) {
  const refreshed = await client.getAccount(accountId);
  const withdraw = signWithdrawTransaction({
    chainId,
    from: accountId,
    nonce: refreshed.nonce,
    assetId: Number(process.env.ENTROPIS_WITHDRAW_ASSET_ID ?? "0"),
    amount: process.env.ENTROPIS_WITHDRAW_AMOUNT ?? "100000000",
    l1Recipient: process.env.ENTROPIS_L1_RECIPIENT,
    gasLimit: 1000,
    maxGasPrice: "1",
    keyPair,
  });
  const withdrawHash = txHash(withdraw);
  await client.submitTx(withdraw);

  if (operatorToken) {
    const operator = new EntropisClient(apiUrl, { adminToken: operatorToken });
    await operator.adminProduceBlock();
  }

  const withdrawalProof = await client.getWithdrawalProof(
    withdrawalId(
      withdrawHash,
      Number(process.env.ENTROPIS_WITHDRAW_ASSET_ID ?? "0"),
      process.env.ENTROPIS_WITHDRAW_AMOUNT ?? "100000000",
      accountId,
      process.env.ENTROPIS_L1_RECIPIENT,
    ),
  );
  const claimMessage = claimWithdrawalTonConnectMessage({
    rollupRootAddress: process.env.ENTROPIS_ROLLUP_ROOT_ADDRESS,
    proof: withdrawalProof,
    amount: "150000000",
  });

  console.log("Send this TON Connect claim message:", claimMessage);
}
