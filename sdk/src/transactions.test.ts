import assert from "node:assert/strict";
import test from "node:test";
import nacl from "tweetnacl";
import {
  accountIdFromKeyPair,
  accountIdFromPublicKey,
  buildTransferTransaction,
  buildWithdrawTransaction,
  deriveAccountId,
  L2_NATIVE_GAS_ASSET,
  signTransferTransaction,
  txHash,
} from "./index.js";
import { hash, TON_RECIPIENT } from "./test_support.js";

test("account helpers derive the same account id from key pair and public key", () => {
  const keyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(7));
  const expected = deriveAccountId(keyPair.publicKey);

  assert.equal(accountIdFromKeyPair(keyPair), expected);
  assert.equal(accountIdFromPublicKey(Buffer.from(keyPair.publicKey).toString("hex")), expected);
  assert.throws(() => accountIdFromPublicKey("aa"), /ed25519 public key must be 32 bytes/);
});

test("transfer helper signs chain-id-bound L2 transactions", () => {
  const keyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(1));
  const base = {
    from: accountIdFromKeyPair(keyPair),
    nonce: 3,
    to: hash(0xbb),
    assetId: L2_NATIVE_GAS_ASSET,
    amount: "1000",
    gasLimit: 500,
    maxGasPrice: "42",
    keyPair,
  };

  const unsigned = buildTransferTransaction({
    ...base,
    chainId: "entropis-testnet",
  });
  const first = signTransferTransaction({ ...base, chainId: "entropis-testnet" });
  const second = signTransferTransaction({ ...base, chainId: "entropis-other" });

  assert.deepEqual(unsigned.kind, {
    Transfer: {
      to: hash(0xbb),
      asset_id: L2_NATIVE_GAS_ASSET,
      amount: "1000",
    },
  });
  assert.equal(first.from, accountIdFromKeyPair(keyPair));
  assert.notEqual(first.signature, second.signature);
  assert.notEqual(txHash(first), txHash(second));
  assert.throws(
    () =>
      buildTransferTransaction({
        ...base,
        chainId: "entropis-testnet",
        nonce: Number.MAX_SAFE_INTEGER + 1,
      }),
    /nonce must be a non-negative safe integer/,
  );
});

test("withdraw helper builds canonical unsigned L2 transaction", () => {
  const tx = buildWithdrawTransaction({
    chainId: "entropis-testnet",
    from: hash(0xaa),
    nonce: 7,
    assetId: 1,
    amount: "200",
    l1Recipient: TON_RECIPIENT,
    gasLimit: 500,
    maxGasPrice: "42",
  });

  assert.deepEqual(tx, {
    chain_id: "entropis-testnet",
    from: hash(0xaa),
    nonce: 7,
    gas_limit: 500,
    max_gas_price: "42",
    kind: {
      Withdraw: {
        asset_id: 1,
        amount: "200",
        l1_recipient: TON_RECIPIENT,
      },
    },
    public_key: null,
    signature: null,
  });
  assert.throws(
    () =>
      buildWithdrawTransaction({
        chainId: "entropis-testnet",
        from: hash(0xaa),
        nonce: 0,
        assetId: 1,
        amount: "0",
        l1Recipient: TON_RECIPIENT,
        gasLimit: 500,
        maxGasPrice: "42",
      }),
    /amount must be non-zero/,
  );
  assert.throws(
    () =>
      buildWithdrawTransaction({
        chainId: "entropis-testnet",
        from: hash(0xaa),
        nonce: 0,
        assetId: 1,
        amount: "1",
        l1Recipient: "not-a-ton-address",
        gasLimit: 500,
        maxGasPrice: "42",
      }),
    /address/i,
  );
});
