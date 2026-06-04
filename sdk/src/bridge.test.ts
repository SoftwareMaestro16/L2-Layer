import assert from "node:assert/strict";
import test from "node:test";
import {
  buildClaimWithdrawalBody,
  claimWithdrawalTonConnectMessage,
  depositTonTonConnectMessage,
  jettonDepositForwardPayload,
  releaseAuthorizedCell,
  RollupRootL1,
  tonDepositForwardPayload,
  withdrawalMerkleProofCell,
} from "./index.js";
import {
  cellFromBase64,
  hash,
  TON_RECIPIENT,
  vectorWithdrawalProof,
  withdrawalLeafFromSeed,
} from "./test_support.js";

test("DepositTon TON Connect message encodes AssetVault body", () => {
  const recipient = hash(0x77);
  const message = depositTonTonConnectMessage({
    vaultAddress: TON_RECIPIENT,
    queryId: 7,
    amount: "100000000",
    l2Recipient: recipient,
  });

  assert.equal(message.address, TON_RECIPIENT);
  assert.equal(message.amount, "100000000");
  const body = cellFromBase64(message.payload);
  const slice = body.beginParse();
  assert.equal(slice.loadUint(32), 0x4c324405);
  assert.equal(slice.loadUintBig(64), 7n);
  assert.equal(slice.loadCoins(), 100000000n);
  assert.equal(slice.loadUintBig(256).toString(16).padStart(64, "0"), recipient);
  slice.endParse();
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

test("release leaf and claim body match Rust withdrawal proof vector", () => {
  const stableLeaf = withdrawalLeafFromSeed(1, "300000000");
  assert.equal(
    releaseAuthorizedCell(stableLeaf).hash().toString("hex"),
    "206ba4b2d3b80535c59d77a2ef1f5342ad31c8b552562a4c38af310bfd5557dc",
  );

  const proof = vectorWithdrawalProof();
  const body = buildClaimWithdrawalBody(proof);
  assert.equal(
    body.toBoc().toString("base64"),
    "te6cckEBBAEA7wACWEwyVwQAAAAAAAAACr2ZyH+oRxIRwfq1NKtWtLX01mLswDfzBZUe7zWNF/rRAQIAlUwyUga9mch/qEcSEcH6tTSrVrS19NZi7MA38wWVHu81jRf60QAAAAGAHJsqnfPpwkoUTWt3Wu1Dm6L5+hF1da3phHxuROFd7e7DkQEVAAAAAAAAAAEAAsADAMMCwPUucWMQT7w9iFkpJ91Ae/tS9ZNmu5qy6qNUmEv1NB75NBfJISFvnHGHIpYzk78U7IGDr8FFWd3wezAsq7KXrAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQJY/TtE=",
  );
  const parsed = RollupRootL1.ClaimWithdrawal.fromSlice(body.beginParse());
  assert.equal(parsed.batchNo, 10n);
  assert.equal(parsed.withdrawalId.toString(16).padStart(64, "0"), proof.leaf.withdrawal_id);
  assert.equal(
    parsed.withdrawalLeaf.hash().toString("hex"),
    "24c9764caf58ca140afd2114c38c10d4285ca6b1fcf008d8c5d7ba6bb9b86e93",
  );
});

test("withdrawal merkle proof helper uses chunked nullable-ref layout", () => {
  const proof = vectorWithdrawalProof().proof;
  const cell = withdrawalMerkleProofCell(proof);
  const slice = cell.beginParse();
  assert.equal(slice.loadUintBig(64), 1n);
  assert.equal(slice.loadUint(16), 2);
  assert.equal(slice.loadBit(), true);
  const chunk = slice.loadRef().beginParse();
  assert.equal(chunk.loadUint(8), 2);
  assert.equal(chunk.loadUintBig(256).toString(16).padStart(64, "0"), proof.siblings[0]);
  assert.equal(chunk.loadUintBig(256).toString(16).padStart(64, "0"), proof.siblings[1]);
  assert.equal(chunk.loadUintBig(256), 0n);
  assert.equal(chunk.loadBit(), false);
  chunk.endParse();
  slice.endParse();
});

test("claim helper builds TON Connect raw message payload", () => {
  const proof = vectorWithdrawalProof();
  const message = claimWithdrawalTonConnectMessage({
    rollupRootAddress: TON_RECIPIENT,
    proof,
    amount: "150000000",
  });

  assert.equal(message.address, TON_RECIPIENT);
  assert.equal(message.amount, "150000000");
  assert.equal(message.payload, buildClaimWithdrawalBody(proof).toBoc().toString("base64"));
});
