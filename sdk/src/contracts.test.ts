import assert from "node:assert/strict";
import test from "node:test";
import { Cell } from "@ton/core";
import nacl from "tweetnacl";
import {
  accountIdFromKeyPair,
  buildCallContractTransaction,
  buildDeployContractTransaction,
  enwalletV5CodeHash,
  enwalletV5InitialState,
  enwalletV5SignedExternalBodyBase64,
  enwalletV5SignedInternalBodyBase64,
  parseEnWalletV5GetterResult,
  readSampleCounterFromAccount,
  sampleCounterIncrementBodyBase64,
  sampleCounterInitialState,
  sampleCounterStorageRoot,
  signEnWalletV5InitTransaction,
  signEnWalletV5CallTransaction,
  signCallContractTransaction,
  signDeployContractTransaction,
  TonL2Client,
  txHash,
  EnWalletV5Generated,
  ENWALLET_V5R1_INTERFACE,
  ENWALLET_V5R1_LABEL,
  ENWALLET_V5R1_TESTNET_WALLET_ID,
  L2_ZERO_ACCOUNT_ID,
  l2RawAddress,
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
    codeBocBase64: sample.code_boc_base64,
    dataBocBase64: sample.data_boc_base64,
    gasLimit: 50,
    maxGasPrice: "1",
    keyPair,
  });
  const unsignedDeploy = buildDeployContractTransaction({
    chainId: "entropis-testnet",
    from,
    nonce: 0,
    contract,
    codeBocBase64: sample.code_boc_base64,
    dataBocBase64: sample.data_boc_base64,
    gasLimit: 50,
    maxGasPrice: "1",
  });

  assert.deepEqual(deploy.kind, unsignedDeploy.kind);
  assert.equal("DeployContract" in deploy.kind, true);
  if (!("DeployContract" in deploy.kind)) {
    throw new Error("expected deploy transaction");
  }
  assert.equal(deploy.kind.DeployContract.contract, contract);
  assert.equal(deploy.kind.DeployContract.code_boc_base64, sample.code_boc_base64);
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

  assert.throws(
    () =>
      buildDeployContractTransaction({
        chainId: "entropis-testnet",
        from,
        nonce: 0,
        contract: L2_ZERO_ACCOUNT_ID,
        codeBocBase64: sample.code_boc_base64,
        dataBocBase64: sample.data_boc_base64,
        gasLimit: 50,
        maxGasPrice: "1",
      }),
    /reserved zero address/,
  );
  assert.throws(
    () =>
      buildCallContractTransaction({
        chainId: "entropis-testnet",
        from: L2_ZERO_ACCOUNT_ID,
        nonce: 0,
        contract,
        bodyBocBase64,
        gasLimit: 50,
        maxGasPrice: "1",
      }),
    /reserved zero address/,
  );
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
      code_boc_base64: sample.code_boc_base64,
      data_boc_base64: sample.data_boc_base64,
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
        code_boc_base64: sample.code_boc_base64,
        data_boc_base64: sample.data_boc_base64,
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

test("EnWallet V5 helpers derive init state and signed wallet body", () => {
  const keyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(7));
  const initial = enwalletV5InitialState({ publicKey: keyPair.publicKey });

  assert.equal(initial.interface, ENWALLET_V5R1_INTERFACE);
  assert.equal(initial.interface_label, ENWALLET_V5R1_LABEL);
  assert.equal(initial.owner_account_id, accountIdFromKeyPair(keyPair));
  assert.equal(initial.code_hash, enwalletV5CodeHash());
  assert.equal(initial.data_hash, initial.storage_root);
  assert.match(initial.wallet_account_id, /^[0-9a-f]{64}$/);
  assert.notEqual(initial.wallet_account_id, initial.owner_account_id);

  const initTx = signEnWalletV5InitTransaction({
    chainId: "entropis-testnet",
    from: initial.owner_account_id,
    nonce: 0,
    gasLimit: 50,
    maxGasPrice: "1",
    keyPair,
  });
  assert.equal("DeployContract" in initTx.kind, true);
  if (!("DeployContract" in initTx.kind)) {
    throw new Error("expected deploy transaction");
  }
  assert.equal(initTx.kind.DeployContract.contract, initial.wallet_account_id);
  assert.equal(initTx.kind.DeployContract.code_boc_base64, initial.code_boc_base64);
  assert.equal(initTx.kind.DeployContract.data_boc_base64, initial.data_boc_base64);

  const callTx = signEnWalletV5CallTransaction({
    chainId: "entropis-testnet",
    from: initial.owner_account_id,
    nonce: 1,
    gasLimit: 50,
    maxGasPrice: "1",
    keyPair,
    walletSeqno: 0,
    walletValidUntil: 4_294_967_295,
  });
  assert.equal("CallContract" in callTx.kind, true);
  if (!("CallContract" in callTx.kind)) {
    throw new Error("expected call transaction");
  }
  assert.equal(callTx.kind.CallContract.contract, initial.wallet_account_id);
  assert.equal(callTx.tx_version, 2);
  assert.equal(callTx.fee_asset_id, 0);

  const internalBody = Cell.fromBoc(
    Buffer.from(
      enwalletV5SignedInternalBodyBase64({
        keyPair,
        validUntil: 4_294_967_295,
        seqno: 0,
      }),
      "base64",
    ),
  )[0].beginParse();
  assert.equal(internalBody.loadUint(32), 0x73696e74);
  assert.equal(internalBody.remainingBits >= 512, true);

  const validUntil = 4_294_967_295;
  const seqno = 0;
  const externalBody = Cell.fromBoc(
    Buffer.from(
      enwalletV5SignedExternalBodyBase64({
        keyPair,
        validUntil,
        seqno,
      }),
      "base64",
    ),
  )[0].beginParse();
  assert.equal(externalBody.loadUint(32), 0x7369676e);
  assert.equal(externalBody.loadUint(32), Number(ENWALLET_V5R1_TESTNET_WALLET_ID));
  assert.equal(externalBody.loadUint(32), validUntil);
  assert.equal(externalBody.loadUint(32), seqno);
  assert.equal(externalBody.loadBoolean(), false);
  assert.equal(externalBody.loadBoolean(), false);
  assert.equal(externalBody.remainingBits, 512);
  assert.equal(externalBody.remainingRefs, 0);

  const unsignedExternal = EnWalletV5Generated.ExternalSignedRequest.toCell(
    EnWalletV5Generated.ExternalSignedRequest.create({
      walletId: ENWALLET_V5R1_TESTNET_WALLET_ID,
      validUntil: BigInt(validUntil),
      seqno: BigInt(seqno),
      outActions: null,
      hasExtraActions: false,
      extraActions: Cell.EMPTY.beginParse(),
    }),
  );
  const signature = externalBody.loadBuffer(64);
  assert.equal(
    nacl.sign.detached.verify(unsignedExternal.hash(), signature, keyPair.publicKey),
    true,
  );

  const parsedSeqno = parseEnWalletV5GetterResult({
    method: "seqno",
    result: {
      interface: ENWALLET_V5R1_INTERFACE,
      interface_label: ENWALLET_V5R1_LABEL,
      result: { type: "uint32", value: "1" },
    },
  });
  assert.deepEqual(parsedSeqno, {
    interface: ENWALLET_V5R1_INTERFACE,
    interface_label: ENWALLET_V5R1_LABEL,
    method: "seqno",
    type: "uint32",
    value: "1",
  });
  assert.throws(
    () =>
      parseEnWalletV5GetterResult({
        method: "seqno",
        result: { type: "uint32", value: "1" },
      }),
    /not an EnWallet V5 getter result/,
  );
});

test("client posts read-only get-method requests", async () => {
  const contract = hash(0x22);
  const calls: Array<{ url: string; init?: RequestInit }> = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: url.toString(), init });
    return new Response(
      JSON.stringify({
        contract,
        contract_raw_address: l2RawAddress(contract),
        contract_friendly_address: "EXtest",
        method: "currentCounter",
        method_id: 1,
        gas_limit: 25,
        gas_used: 0,
        vm_exit_code: 0,
        result: { type: "uint64", value: "8" },
        source: "l2_state",
        read_only: true,
        state_root: hash(0x33),
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  }) as typeof fetch;

  try {
    const client = new TonL2Client("http://127.0.0.1:8080");
    const response = await client.getContractMethod(contract, {
      method: "currentCounter",
      stackBocBase64: "te6ccgEBAQEAAgAAAA==",
      gasLimit: "25",
    });

    assert.equal(response.read_only, true);
    assert.equal(calls.length, 1);
    assert.equal(
      calls[0].url,
      `http://127.0.0.1:8080/v1/contract/${encodeURIComponent(l2RawAddress(contract))}/get-method`,
    );
    assert.equal(calls[0].init?.method, "POST");
    assert.deepEqual(JSON.parse(String(calls[0].init?.body)), {
      method: "currentCounter",
      stack_boc_base64: "te6ccgEBAQEAAgAAAA==",
      gas_limit: 25,
    });
  } finally {
    globalThis.fetch = previousFetch;
  }
});
