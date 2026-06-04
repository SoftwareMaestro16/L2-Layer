import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { Cell } from "@ton/core";
import { L2_NATIVE_GAS_ASSET, withdrawalId } from "./index.js";
import type { SignedL2Transaction, WithdrawalProofLeaf, WithdrawalProofResponse } from "./types.js";

export const TON_RECIPIENT = "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR";

export function vectorTransaction(): SignedL2Transaction {
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

export function hash(byte: number): string {
  return byte.toString(16).padStart(2, "0").repeat(32);
}

export function withdrawalLeafFromSeed(seed: number, amount: string): WithdrawalProofLeaf {
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

export function vectorWithdrawalProof(): WithdrawalProofResponse {
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

export function cellFromBase64(payload: string): Cell {
  return Cell.fromBoc(Buffer.from(payload, "base64"))[0];
}

function sha256Bytes(bytes: number[]): string {
  return createHash("sha256").update(Buffer.from(bytes)).digest("hex");
}
