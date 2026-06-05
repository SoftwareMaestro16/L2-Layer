import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import { Address, Cell } from "@ton/core";
import nacl from "tweetnacl";
import {
  accountIdFromKeyPair,
  accountIdFromPublicKey,
  accountLeafHash,
  blockHeaderHash,
  buildClaimWithdrawalBody,
  buildRotatePublicKeyTransaction,
  buildTransferTransaction,
  buildWithdrawTransaction,
  canonicalBatchDataHash,
  claimWithdrawalTonConnectMessage,
  depositJettonTonConnectMessage,
  depositTonTonConnectMessage,
  encodeJettonDepositTransferBody,
  encodeReceipt,
  deriveAccountId,
  encodeSignedTransaction,
  encodeUnsignedTransaction,
  EntropisApiError,
  EntropisClient,
  jettonDepositForwardPayload,
  JETTON_TRANSFER_OPCODE,
  l2RawAddress,
  l2UserFriendlyAddress,
  isL2ZeroAddress,
  L2_NATIVE_GAS_ASSET,
  L2_TRANSACTION_KIND_VERSION_V1,
  L2_TX_DOMAIN_SEPARATOR,
  L2_TX_VERSION_V2,
  L2_ZERO_ACCOUNT_ID,
  L2_ZERO_FRIENDLY_ADDRESS,
  L2_ZERO_RAW_ADDRESS,
  parseL2Address,
  receiptLeafHash,
  releaseAuthorizedCell,
  RollupRootL1,
  signRotatePublicKeyTransaction,
  signTransferTransaction,
  tonDepositForwardPayload,
  txHash,
  withdrawalMerkleProofCell,
  withdrawalId,
  withdrawalLeafHash,
  type AccountLeaf,
  type Receipt,
  type SignedL2Transaction,
  type WithdrawalLeaf,
  type WithdrawalProofResponse,
} from "./index.js";
import { EntropisAdminClient } from "./admin.js";
import { BrowserEntropisClient, createEntropisWalletAccount } from "./browser.js";

const TON_RECIPIENT = "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR";

test("unsigned transaction encoding matches Rust golden vector", () => {
  assert.equal(
    Buffer.from(encodeUnsignedTransaction(vectorTransaction())).toString("hex"),
    "454c32430101000200000011656e74726f7069732e6c322e74782e763200000010656e74726f7069732d746573746e657401aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000007000000000000006300000000000001f40000000000000000000000000000002a0000000001dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd000102bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00000000000000000000000000000000000003e8",
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

  assert.equal(txHash(tx), "039718f82070163d8d92c41fea4e14baf42be6b5ceb0d10a0d66d095eee77590");
  assert.equal(
    receiptLeafHash(receipt),
    "002b7a3abb022a944ab4060db65f6df449f2875528dbf4cd7047e7ee64281bb0",
  );
  assert.equal(
    withdrawalLeafHash(withdrawal),
    "978ae37e92dca1a024d86eb82b37cf474766d26c448460de0f247ffe140cd5d0",
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
    "dbc739765aaba3517b6ffe7cea1e5f7f6ab5711e677cc95a3f440be6ac2b425d",
  );
  assert.equal(
    accountLeafHash(hash(0xaa), account),
    "2b283b553c4d56e5ee8054b55601397d27c9ce00b4620a7001f2f44c538e9331",
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

test("receipt events are consensus encoded", () => {
  const tx_hash = hash(0xaa);
  const plain: Receipt = {
    tx_hash,
    status: "Applied",
    gas_charged: "10",
    reason: null,
    withdrawal_id: null,
  };
  const withEvent: Receipt = {
    ...plain,
    events: [
      {
        ContractCalled: {
          contract: hash(0x01),
          caller: hash(0x02),
          body_hash: hash(0x03),
        },
      },
      {
        FeeDistributed: {
          asset_id: L2_NATIVE_GAS_ASSET,
          total_amount: "100",
          sequencer_amount: "90",
          operator_amount: "7",
          treasury_amount: "3",
          sequencer_reward_account: hash(0x04),
          operator_fee_account: hash(0x05),
          treasury_fee_account: hash(0x06),
        },
      },
    ],
  };

  assert.notEqual(receiptLeafHash(plain), receiptLeafHash(withEvent));
  assert.ok(encodeReceipt(withEvent).length > encodeReceipt(plain).length);
});

test("account helpers derive the same account id from key pair and public key", () => {
  const keyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(7));
  const expected = deriveAccountId(keyPair.publicKey);

  assert.equal(accountIdFromKeyPair(keyPair), expected);
  assert.equal(accountIdFromPublicKey(Buffer.from(keyPair.publicKey).toString("hex")), expected);
  assert.throws(() => accountIdFromPublicKey("aa"), /ed25519 public key must be 32 bytes/);
});

test("L2 address helpers format raw and user-friendly addresses", () => {
  const accountId = hash(0x42);
  const raw = l2RawAddress(accountId);
  const friendly = l2UserFriendlyAddress(accountId);

  assert.equal(raw, `8:${accountId}`);
  assert.equal(raw.length, 66);
  assert.equal(friendly.length, 48);
  assert.equal(friendly.slice(0, 2), "EX");
  assert.equal(parseL2Address(raw), accountId);
  assert.equal(parseL2Address(friendly), accountId);
  assert.equal(parseL2Address(accountId), accountId);
  assert.throws(() => parseL2Address(`${friendly.slice(0, 47)}A`), /checksum|invalid/i);
});

test("L2 zero address is stable and reserved by helpers", async () => {
  assert.equal(L2_ZERO_ACCOUNT_ID, "0".repeat(64));
  assert.equal(L2_ZERO_RAW_ADDRESS, `8:${"0".repeat(64)}`);
  assert.equal(L2_ZERO_FRIENDLY_ADDRESS, "EXgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGdh");
  assert.equal(l2RawAddress(L2_ZERO_ACCOUNT_ID), L2_ZERO_RAW_ADDRESS);
  assert.equal(l2UserFriendlyAddress(L2_ZERO_ACCOUNT_ID), L2_ZERO_FRIENDLY_ADDRESS);
  assert.equal(parseL2Address(L2_ZERO_RAW_ADDRESS), L2_ZERO_ACCOUNT_ID);
  assert.equal(parseL2Address(L2_ZERO_FRIENDLY_ADDRESS), L2_ZERO_ACCOUNT_ID);
  assert.equal(isL2ZeroAddress(L2_ZERO_FRIENDLY_ADDRESS), true);

  assert.throws(() => tonDepositForwardPayload(L2_ZERO_ACCOUNT_ID), /reserved zero address/);
  assert.throws(
    () =>
      depositTonTonConnectMessage({
        vaultAddress: TON_RECIPIENT,
        queryId: 7,
        amount: "100000000",
        l2Recipient: L2_ZERO_FRIENDLY_ADDRESS,
      }),
    /reserved zero address/,
  );
  assert.throws(
    () =>
      buildTransferTransaction({
        chainId: "entropis-testnet",
        from: hash(0xaa),
        nonce: 0,
        to: L2_ZERO_RAW_ADDRESS,
        assetId: 0,
        amount: "1",
        gasLimit: 10,
        maxGasPrice: 1,
      }),
    /reserved zero address/,
  );
  assert.throws(
    () =>
      buildWithdrawTransaction({
        chainId: "entropis-testnet",
        from: L2_ZERO_RAW_ADDRESS,
        nonce: 0,
        assetId: 0,
        amount: "1",
        l1Recipient: TON_RECIPIENT,
        gasLimit: 20,
        maxGasPrice: 1,
      }),
    /reserved zero address/,
  );

  const previousFetch = globalThis.fetch;
  globalThis.fetch = (async () => {
    throw new Error("zero faucet request must fail before fetch");
  }) as typeof fetch;
  try {
    const client = new EntropisAdminClient("http://127.0.0.1:8080", { adminToken: "operator" });
    await assert.rejects(
      client.requestEntFaucet(L2_ZERO_FRIENDLY_ADDRESS),
      /reserved zero address/,
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("L2 user-friendly addresses can use the full base64url alphabet after EX", () => {
  const observed = new Set<string>();
  for (let index = 0; index < 10_000; index += 1) {
    const accountId = createHash("sha256").update(`entropis-${index}`).digest("hex");
    const friendly = l2UserFriendlyAddress(accountId);
    for (const char of friendly.slice(2)) {
      observed.add(char);
    }
  }

  assert.deepEqual(
    [...observed].sort().join(""),
    "-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz",
  );
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

test("Jetton deposit TON Connect message encodes TEP-74 transfer body", () => {
  const recipient = hash(0x77);
  const message = depositJettonTonConnectMessage({
    jettonWalletAddress: TON_RECIPIENT,
    vaultAddress: TON_RECIPIENT,
    responseAddress: TON_RECIPIENT,
    queryId: 8,
    jettonAmount: "123456",
    forwardTonAmount: "50000000",
    tonAmount: "100000000",
    l2Recipient: recipient,
  });

  assert.equal(message.address, TON_RECIPIENT);
  assert.equal(message.amount, "100000000");

  const body = cellFromBase64(message.payload);
  const slice = body.beginParse();
  assert.equal(slice.loadUint(32), JETTON_TRANSFER_OPCODE);
  assert.equal(slice.loadUintBig(64), 8n);
  assert.equal(slice.loadCoins(), 123456n);
  assert.equal(slice.loadAddress().equals(Address.parse(TON_RECIPIENT)), true);
  assert.equal(slice.loadAddress().equals(Address.parse(TON_RECIPIENT)), true);
  assert.equal(slice.loadBit(), false);
  assert.equal(slice.loadCoins(), 50000000n);
  assert.equal(slice.loadBit(), true);

  const payload = slice.loadRef().beginParse();
  assert.equal(payload.loadUintBig(256).toString(16).padStart(64, "0"), recipient);
  payload.endParse();
  slice.endParse();
});

test("Jetton deposit transfer helper rejects unsafe amounts and recipients", () => {
  assert.throws(
    () =>
      encodeJettonDepositTransferBody({
        jettonWalletAddress: TON_RECIPIENT,
        vaultAddress: TON_RECIPIENT,
        responseAddress: TON_RECIPIENT,
        queryId: 8,
        jettonAmount: "123456",
        forwardTonAmount: "0",
        tonAmount: "100000000",
        l2Recipient: hash(0x77),
      }),
    /forwardTonAmount must be non-zero/,
  );
  assert.throws(
    () =>
      encodeJettonDepositTransferBody({
        jettonWalletAddress: TON_RECIPIENT,
        vaultAddress: "not-a-ton-address",
        responseAddress: TON_RECIPIENT,
        queryId: 8,
        jettonAmount: "123456",
        forwardTonAmount: "1",
        tonAmount: "100000000",
        l2Recipient: hash(0x77),
      }),
    /address/i,
  );
  assert.throws(
    () =>
      encodeJettonDepositTransferBody({
        jettonWalletAddress: TON_RECIPIENT,
        vaultAddress: TON_RECIPIENT,
        responseAddress: TON_RECIPIENT,
        queryId: 8,
        jettonAmount: "123456",
        forwardTonAmount: "1",
        tonAmount: "100000000",
        l2Recipient: L2_ZERO_ACCOUNT_ID,
      }),
    /reserved zero address/,
  );
  assert.throws(
    () =>
      encodeJettonDepositTransferBody({
        jettonWalletAddress: TON_RECIPIENT,
        vaultAddress: TON_RECIPIENT,
        responseAddress: TON_RECIPIENT,
        queryId: (1n << 64n).toString(10),
        jettonAmount: "123456",
        forwardTonAmount: "1",
        tonAmount: "100000000",
        l2Recipient: hash(0x77),
      }),
    /queryId exceeds uint64/,
  );
  assert.throws(
    () =>
      encodeJettonDepositTransferBody({
        jettonWalletAddress: TON_RECIPIENT,
        vaultAddress: TON_RECIPIENT,
        responseAddress: TON_RECIPIENT,
        queryId: 8,
        jettonAmount: (1n << 120n).toString(10),
        forwardTonAmount: "1",
        tonAmount: "100000000",
        l2Recipient: hash(0x77),
      }),
    /jettonAmount exceeds uint120/,
  );
  assert.throws(
    () =>
      depositJettonTonConnectMessage({
        jettonWalletAddress: "not-a-ton-address",
        vaultAddress: TON_RECIPIENT,
        responseAddress: TON_RECIPIENT,
        queryId: 8,
        jettonAmount: "123456",
        forwardTonAmount: "1",
        tonAmount: "100000000",
        l2Recipient: hash(0x77),
      }),
    /address/i,
  );
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

test("public key rotation helper keeps account id and signs new key binding", () => {
  const oldKeyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(1));
  const newKeyPair = nacl.sign.keyPair.fromSeed(new Uint8Array(32).fill(2));
  const accountId = accountIdFromKeyPair(oldKeyPair);
  const newPublicKey = Buffer.from(newKeyPair.publicKey).toString("hex");

  const unsigned = buildRotatePublicKeyTransaction({
    chainId: "entropis-testnet",
    from: accountId,
    nonce: 4,
    newPublicKey,
    gasLimit: 10,
    maxGasPrice: 1,
  });
  const signed = signRotatePublicKeyTransaction({
    chainId: "entropis-testnet",
    from: accountId,
    nonce: 4,
    newPublicKey: newKeyPair.publicKey,
    gasLimit: 10,
    maxGasPrice: 1,
    keyPair: oldKeyPair,
  });

  assert.deepEqual(unsigned.kind, { RotatePublicKey: { new_public_key: newPublicKey } });
  assert.equal(signed.from, accountId);
  assert.equal("RotatePublicKey" in signed.kind, true);
  if (!("RotatePublicKey" in signed.kind)) {
    throw new Error("expected RotatePublicKey transaction");
  }
  assert.equal(signed.kind.RotatePublicKey.new_public_key, newPublicKey);
  assert.notEqual(signed.public_key, signed.kind.RotatePublicKey.new_public_key);
  assert.throws(
    () =>
      buildRotatePublicKeyTransaction({
        chainId: "entropis-testnet",
        from: accountId,
        nonce: 4,
        newPublicKey: "aa",
        gasLimit: 10,
        maxGasPrice: 1,
      }),
    /newPublicKey must be 32 bytes/,
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
    tx_version: L2_TX_VERSION_V2,
    domain_separator: L2_TX_DOMAIN_SEPARATOR,
    chain_id: "entropis-testnet",
    from: hash(0xaa),
    nonce: 7,
    valid_until_block: Number.MAX_SAFE_INTEGER,
    gas_limit: 500,
    max_gas_price: "42",
    fee_asset_id: L2_NATIVE_GAS_ASSET,
    memo_hash: null,
    transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
    kind: {
      Withdraw: {
        asset_id: 1,
        amount: "200",
        l1_recipient: TON_RECIPIENT,
      },
    },
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
  assert.equal(
    parsed.withdrawalId.toString(16).padStart(64, "0"),
    proof.leaf.withdrawal_id,
  );
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
  assert.equal(
    chunk.loadUintBig(256).toString(16).padStart(64, "0"),
    proof.siblings[0],
  );
  assert.equal(
    chunk.loadUintBig(256).toString(16).padStart(64, "0"),
    proof.siblings[1],
  );
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

test("Entropis client maps faucet requests and API errors safely", async () => {
  const previousFetch = globalThis.fetch;
  const accountId = hash(0xaa);
  const calls: Array<{ url: string; init?: RequestInit }> = [];
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: input.toString(), init });
    if (input.toString().endsWith("/v1/admin/faucet/ent")) {
      assert.equal((init?.headers as Record<string, string>).authorization, "Bearer operator");
      assert.deepEqual(JSON.parse(init?.body as string), { account_id: l2RawAddress(accountId) });
      return new Response(
        JSON.stringify({
          account_id: accountId,
          account_raw_address: l2RawAddress(accountId),
          account_friendly_address: l2UserFriendlyAddress(accountId),
          amount_ent: "1000",
          amount_base_units: "1000000000000",
          deposit_id: hash(0xdd),
          granted: true,
        }),
        { status: 200 },
      );
    }
    return new Response(JSON.stringify({ error: "nonce_locked" }), { status: 409 });
  }) as typeof fetch;

  try {
    const client = new EntropisAdminClient("http://127.0.0.1:8080/", { adminToken: "operator" });
    const faucet = await client.requestEntFaucet(accountId);
    assert.equal(client.baseUrl, "http://127.0.0.1:8080");
    assert.equal(faucet.granted, true);
    assert.equal(calls[0].url, "http://127.0.0.1:8080/v1/admin/faucet/ent");

    await assert.rejects(
      client.submitTx(vectorTransaction()),
      (error) =>
        error instanceof EntropisApiError &&
        error.status === 409 &&
        error.publicMessage === "nonce_locked",
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test("browser entrypoint creates EnWallet accounts without admin helpers", async () => {
  const account = await createEntropisWalletAccount();
  const client = new BrowserEntropisClient("http://127.0.0.1:8080/");

  assert.equal(account.recoveryWords.length, 24);
  assert.equal(account.ownerAccountId, account.initialState.owner_account_id);
  assert.equal(account.walletAccountId, account.initialState.wallet_account_id);
  assert.equal(account.rawAddress.startsWith("8:"), true);
  assert.equal(account.userFriendlyAddress.startsWith("EX"), true);
  assert.equal("requestEntFaucet" in client, false);
  assert.equal("produceBlock" in client, false);
  assert.equal("devDeposit" in client, false);
});

test("Entropis client polls transaction receipt lifecycle", async () => {
  const previousFetch = globalThis.fetch;
  const txHashValue = hash(0xab);
  const calls: string[] = [];
  globalThis.fetch = (async (input: string | URL | Request) => {
    calls.push(input.toString());
    if (calls.length === 1) {
      return new Response(JSON.stringify({ error: "transaction not found" }), { status: 404 });
    }
    if (calls.length === 2) {
      return new Response(
        JSON.stringify({
          tx_hash: txHashValue,
          status: "pending",
          transaction: null,
          receipt: null,
          block: null,
          finality: null,
        }),
        { status: 200 },
      );
    }
    return new Response(
      JSON.stringify({
        tx_hash: txHashValue,
        status: "finalized",
        transaction: null,
        receipt: {
          status: "applied",
          gas_charged: "10",
          reason: null,
          withdrawal_id: null,
          events: [],
          contract_logs: [],
        },
        block: {
          height: 7,
          timestamp: 123,
          block_hash: hash(0xbc),
          tx_index: 0,
        },
        finality: {
          block_height: 7,
          block_hash: hash(0xbc),
          batch_no: 8,
          committed: true,
          finalized: true,
          commit: {
            status: "confirmed",
            attempts: 1,
            message_hash: null,
            message_hash_norm: hash(0xcd),
          },
          finalization: {
            status: "finalized",
            attempts: 1,
            finalize_after_unix: 124,
            message_hash: null,
            message_hash_norm: hash(0xde),
          },
        },
      }),
      { status: 200 },
    );
  }) as typeof fetch;

  try {
    const client = new EntropisClient("http://127.0.0.1:8080/");
    const receipt = await client.waitForTxReceipt(txHashValue, {
      desiredStatus: "finalized",
      intervalMs: 1,
      timeoutMs: 500,
    });

    assert.equal(receipt.status, "finalized");
    assert.equal(receipt.receipt?.gas_charged, "10");
    assert.equal(receipt.finality?.finalization?.status, "finalized");
    assert.deepEqual(calls, [
      `http://127.0.0.1:8080/v1/receipt/${txHashValue}`,
      `http://127.0.0.1:8080/v1/receipt/${txHashValue}`,
      `http://127.0.0.1:8080/v1/receipt/${txHashValue}`,
    ]);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

function vectorTransaction(): SignedL2Transaction {
  return {
    tx_version: L2_TX_VERSION_V2,
    domain_separator: L2_TX_DOMAIN_SEPARATOR,
    chain_id: "entropis-testnet",
    from: hash(0xaa),
    nonce: 7,
    valid_until_block: 99,
    gas_limit: 500,
    max_gas_price: "42",
    fee_asset_id: L2_NATIVE_GAS_ASSET,
    memo_hash: hash(0xdd),
    transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
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

function withdrawalLeafFromSeed(seed: number, amount: string): WithdrawalLeaf {
  const txHash = sha256Bytes([seed]);
  const l2Sender = sha256Bytes([seed, 1]);
  return {
    withdrawal_id: withdrawalId(txHash, 1, amount, l2Sender, TON_RECIPIENT),
    asset_id: 1,
    amount,
    l2_sender: l2Sender,
    l1_recipient: TON_RECIPIENT,
  };
}

function vectorWithdrawalProof(): WithdrawalProofResponse {
  const leaf = withdrawalLeafFromSeed(2, "200");
  assert.equal(
    leaf.withdrawal_id,
    "bd99c87fa8471211c1fab534ab56b4b5f4d662ecc037f305951eef358d17fad1",
  );
  return {
    block_height: 9,
    withdrawal_root: "d5e8e681563ae874899124c32b8bb43072a4d95e0b05b2bf9ddda9ce9d5b62cf",
    leaf,
    proof: {
      leaf_index: 1,
      siblings: [
        "c0f52e7163104fbc3d88592927dd407bfb52f59366bb9ab2eaa354984bf5341e",
        "f93417c921216f9c718722963393bf14ec8183afc14559ddf07b302cabb297ac",
      ],
    },
  };
}

function sha256Bytes(bytes: number[]): string {
  return createHash("sha256").update(Buffer.from(bytes)).digest("hex");
}

function cellFromBase64(payload: string): Cell {
  return Cell.fromBoc(Buffer.from(payload, "base64"))[0];
}
