# Entropis SDK

TypeScript helpers for Entropis L2 wallets, demos, and operator scripts. The SDK
does not bake in an API host and does not store wallet or admin secrets.

## L2 Addresses And Fees

ENT is the L2-native gas token. SDK transactions use asset id `0` for gas fees,
and `maxGasPrice` is denominated in ENT base units.

The SDK accepts L2 accounts/contracts as:

- raw: `8:<64 lowercase hex chars>`
- user-friendly: `EX...` base64url, 48 chars total
- legacy bare 64-hex account ids for local compatibility

## Wallet Flow

```ts
import nacl from "tweetnacl";
import {
  EntropisClient,
  accountIdFromKeyPair,
  depositJettonTonConnectMessage,
  depositTonTonConnectMessage,
  signTransferTransaction,
} from "@ton-l2-rollup/sdk";

const client = new EntropisClient("http://127.0.0.1:8080");
const keyPair = nacl.sign.keyPair();
const accountId = accountIdFromKeyPair(keyPair);
const account = await client.getAccount(accountId);

const transfer = signTransferTransaction({
  chainId: "entropis-testnet",
  from: accountId,
  nonce: account.nonce,
  to: "<recipient l2 account id hex>",
  assetId: 0,
  amount: "1000000000",
  gasLimit: 1000,
  maxGasPrice: "1",
  keyPair,
});

await client.submitTx(transfer);

const depositMessage = depositTonTonConnectMessage({
  vaultAddress: "<AssetVault testnet address>",
  queryId: Date.now(),
  amount: "100000000",
  l2Recipient: accountId,
});

const jettonDepositMessage = depositJettonTonConnectMessage({
  jettonWalletAddress: "<user Jetton wallet address>",
  vaultAddress: "<AssetVault testnet address>",
  responseAddress: "<user TON testnet address>",
  queryId: Date.now(),
  jettonAmount: "1000000",
  forwardTonAmount: "50000000",
  tonAmount: "100000000",
  l2Recipient: accountId,
});
```

Use `depositMessage` or `jettonDepositMessage` as a raw TON Connect `messages[]`
entry. TON Connect raw messages carry a user-friendly address, nanotons as a
decimal string, and a base64 BoC payload. Jetton deposits send a TEP-74
`transfer` to the user's Jetton wallet with a canonical ref `forward_payload`
containing the L2 account id.

## Withdrawal Claim

```ts
import { claimWithdrawalTonConnectMessage } from "@ton-l2-rollup/sdk";

const proof = await client.getWithdrawalProof("<withdrawal id hex>");
const claimMessage = claimWithdrawalTonConnectMessage({
  rollupRootAddress: "<RollupRoot testnet address>",
  proof,
  amount: "150000000",
});
```

The node returns withdrawal proofs only after the related batch is finalized. A
pre-finalization request returns HTTP `409`.

## Sample L2 Counter

The SDK can build the bounded sample contract flow used by the prototype TVM
adapter:

```ts
import {
  sampleCounterInitialState,
  sampleCounterIncrementBodyBase64,
  signDeployContractTransaction,
  signCallContractTransaction,
} from "@ton-l2-rollup/sdk";

const initial = sampleCounterInitialState(0);
const deploy = signDeployContractTransaction({
  chainId: "entropis-testnet",
  from: accountId,
  nonce: account.nonce,
  contract: "<32-byte contract id hex>",
  codeHash: initial.code_hash,
  dataHash: initial.data_hash,
  storageRoot: initial.storage_root,
  gasLimit: 50,
  maxGasPrice: "1",
  keyPair,
});

const call = signCallContractTransaction({
  chainId: "entropis-testnet",
  from: accountId,
  nonce: account.nonce + 1,
  contract: "<32-byte contract id hex>",
  bodyBocBase64: sampleCounterIncrementBodyBase64(1),
  gasLimit: 50,
  maxGasPrice: "1",
  keyPair,
});
```

Run `sdk/examples/l2-counter-sample.mjs` after building the SDK for a local
deploy/call/read demo. Unsupported contract code hashes remain fail-closed.

## Operator Faucet

The ENT faucet is admin-only in the MVP. Use it from an operator script or demo
backend, not from browser code:

```ts
const operatorClient = new EntropisClient("http://127.0.0.1:8080", {
  adminToken: process.env.ENTROPIS_ADMIN_TOKEN,
});

await operatorClient.requestEntFaucet(accountId);
```
