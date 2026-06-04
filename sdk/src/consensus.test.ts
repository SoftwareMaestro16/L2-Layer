import assert from "node:assert/strict";
import test from "node:test";
import {
  accountLeafHash,
  blockHeaderHash,
  canonicalBatchDataHash,
  encodeSignedTransaction,
  encodeUnsignedTransaction,
  jettonDepositForwardPayload,
  L2_NATIVE_GAS_ASSET,
  receiptLeafHash,
  tonDepositForwardPayload,
  txHash,
  withdrawalId,
  withdrawalLeafHash,
  type AccountLeaf,
  type Receipt,
  type SignedL2Transaction,
  type WithdrawalLeaf,
} from "./index.js";

test("unsigned transaction encoding matches Rust golden vector", () => {
  assert.equal(
    Buffer.from(encodeUnsignedTransaction(vectorTransaction())).toString("hex"),
    "454c3243010100000010656e74726f7069732d746573746e657401aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa000000000000000700000000000001f40000000000000000000000000000002a02bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00000000000000000000000000000000000003e8",
  );
});

test("consensus hashes match Rust golden vectors", () => {
  const tx = vectorTransaction();
  const receipt: Receipt = {
    tx_hash: txHash(tx),
    status: "Applied",
    gas_charged: "10",
    reason: null,
    withdrawal_id: hash(0xcc),
  };
  const withdrawal: WithdrawalLeaf = {
    withdrawal_id: withdrawalId(
      txHash(tx),
      L2_NATIVE_GAS_ASSET,
      "55",
      hash(0xaa),
      "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c",
    ),
    asset_id: L2_NATIVE_GAS_ASSET,
    amount: "55",
    l2_sender: hash(0xaa),
    l1_recipient: "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c",
  };
  const dataHash = canonicalBatchDataHash([tx], [receipt]);
  const account: AccountLeaf = {
    nonce: 3,
    balances: [
      { asset_id: L2_NATIVE_GAS_ASSET, balance: "1000" },
      { asset_id: 2, balance: "500" },
    ],
    code_hash: hash(0x11),
    data_hash: hash(0x12),
    storage_root: hash(0x13),
    last_lt: 9,
  };

  assert.equal(txHash(tx), "c1a6de1d5b776bdd51ab0fcba6bf4ccb62fd3e317b1a3b485cb7f470d9f3a8ac");
  assert.equal(
    receiptLeafHash(receipt),
    "536c7264a2bc9e0659287068183431b452c614df614bc82f0f25d37b001b8d43",
  );
  assert.equal(
    withdrawalLeafHash(withdrawal),
    "00164447b3c4fb77bf5a9c2bf179782ef7cc6074ce3057ee6d68feb9d6f5c75e",
  );
  assert.equal(
    blockHeaderHash({
      height: 9,
      prev_block_hash: hash(0x01),
      prev_state_root: hash(0x02),
      state_root: hash(0x03),
      tx_root: hash(0x04),
      receipt_root: hash(0x05),
      withdrawal_root: hash(0x06),
      data_hash: dataHash,
      timestamp: 777,
    }),
    "9ee765a283d11084ffb5f0819afbf866f70a3e44ca981048c5705f7dbb1417ba",
  );
  assert.equal(
    accountLeafHash(hash(0xaa), account),
    "191eda257e6182c35676db70e20e54180e2a7f9eec6cddd4ae5c72a2882f97e9",
  );
});

test("signed auth fields are canonical but excluded from tx hash", () => {
  const first = {
    ...vectorTransaction(),
    public_key: "aa".repeat(32),
    signature: "bb".repeat(64),
  };
  const second = {
    ...vectorTransaction(),
    public_key: "cc".repeat(32),
    signature: "dd".repeat(64),
  };

  assert.equal(txHash(first), txHash(second));
  assert.notDeepEqual(encodeSignedTransaction(first), encodeSignedTransaction(second));
});

test("non canonical account and hash inputs are rejected", () => {
  const account: AccountLeaf = {
    nonce: 0,
    balances: [
      { asset_id: 1, balance: "10" },
      { asset_id: 1, balance: "20" },
    ],
    code_hash: hash(0x11),
    data_hash: hash(0x12),
    storage_root: hash(0x13),
    last_lt: 0,
  };
  const invalidHashTx = {
    ...vectorTransaction(),
    from: "",
  };

  assert.throws(() => accountLeafHash(hash(0xaa), account), /duplicate account balance/);
  assert.throws(() => txHash(invalidHashTx), /expected 32-byte hex string/);
});

test("jetton deposit payload encodes canonical l2 recipient", () => {
  const recipient = hash(0x77);
  const jettonPayload = jettonDepositForwardPayload(recipient);
  const tonPayload = tonDepositForwardPayload(recipient);

  assert.equal(jettonPayload.toBoc().toString("hex"), tonPayload.toBoc().toString("hex"));
  const slice = jettonPayload.beginParse();
  assert.equal(slice.loadUintBig(256).toString(16).padStart(64, "0"), recipient);
  assert.equal(slice.remainingBits, 0);
  assert.equal(slice.remainingRefs, 0);
});

function vectorTransaction(): SignedL2Transaction {
  return {
    chain_id: "entropis-testnet",
    from: hash(0xaa),
    nonce: 7,
    gas_limit: 500,
    max_gas_price: "42",
    kind: {
      Transfer: {
        to: hash(0xbb),
        asset_id: L2_NATIVE_GAS_ASSET,
        amount: "1000",
      },
    },
    public_key: null,
    signature: null,
  };
}

function hash(byte: number): string {
  return byte.toString(16).padStart(2, "0").repeat(32);
}
