import assert from "node:assert/strict";
import test from "node:test";
import nacl from "tweetnacl";
import {
  accountIdFromKeyPair,
  buildCallContractTransaction,
  buildDeployContractTransaction,
  readSampleCounterFromAccount,
  sampleCounterCodeHash,
  sampleCounterIncrementBodyBase64,
  sampleCounterInitialState,
  sampleCounterStorageRoot,
  signCallContractTransaction,
  signDeployContractTransaction,
  txHash,
} from "./index.js";

function hash(byte: number): string {
  return Buffer.alloc(32, byte).toString("hex");
}

test("deploy and call contract helpers encode canonical L2 transactions", () => {
  const keyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(9));
  const from = accountIdFromKeyPair(keyPair);
  const contract = hash(0x55);
  const sample = sampleCounterInitialState(0);
  const deploy = signDeployContractTransaction({
    chainId: "entropis-testnet",
    from,
    nonce: 0,
    contract,
    codeHash: sample.code_hash,
    dataHash: sample.data_hash,
    storageRoot: sample.storage_root,
    gasLimit: 50,
    maxGasPrice: "1",
    keyPair,
  });
  const unsignedDeploy = buildDeployContractTransaction({
    chainId: "entropis-testnet",
    from,
    nonce: 0,
    contract,
    codeHash: sample.code_hash,
    dataHash: sample.data_hash,
    storageRoot: sample.storage_root,
    gasLimit: 50,
    maxGasPrice: "1",
  });

  assert.deepEqual(deploy.kind, unsignedDeploy.kind);
  assert.equal("DeployContract" in deploy.kind, true);
  if (!("DeployContract" in deploy.kind)) {
    throw new Error("expected deploy transaction");
  }
  assert.equal(deploy.kind.DeployContract.contract, contract);
  assert.equal(deploy.kind.DeployContract.code_hash, sampleCounterCodeHash());
  assert.equal(typeof deploy.signature, "string");

  const bodyBocBase64 = sampleCounterIncrementBodyBase64(3);
  const call = signCallContractTransaction({
    chainId: "entropis-testnet",
    from,
    nonce: 1,
    contract,
    bodyBocBase64,
    gasLimit: 50,
    maxGasPrice: "1",
    keyPair,
  });
  const unsignedCall = buildCallContractTransaction({
    chainId: "entropis-testnet",
    from,
    nonce: 1,
    contract,
    bodyBocBase64,
    gasLimit: 50,
    maxGasPrice: "1",
  });

  assert.deepEqual(call.kind, unsignedCall.kind);
  assert.notEqual(txHash(deploy), txHash(call));
});

test("sample counter storage helpers decode account state and reject mismatches", () => {
  const sample = sampleCounterInitialState(12);
  assert.equal(sample.storage_root, sampleCounterStorageRoot(12));
  assert.equal(
    readSampleCounterFromAccount({
      nonce: 0,
      balances: {},
      code_hash: sample.code_hash,
      data_hash: sample.data_hash,
      storage_root: sample.storage_root,
      last_lt: 0,
    }),
    12,
  );
  assert.throws(
    () =>
      readSampleCounterFromAccount({
        nonce: 0,
        balances: {},
        code_hash: sample.code_hash,
        data_hash: hash(0x44),
        storage_root: sample.storage_root,
        last_lt: 0,
      }),
    /data hash mismatch/,
  );
  assert.throws(
    () =>
      buildCallContractTransaction({
        chainId: "entropis-testnet",
        from: hash(0x01),
        nonce: 0,
        contract: hash(0x02),
        bodyBocBase64: "***not-base64***",
        gasLimit: 50,
        maxGasPrice: "1",
      }),
    /single-root TON BoC/,
  );
});
