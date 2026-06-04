import { describe, expect, it } from "vitest";
import { accountSchema, transactionDetailSchema } from "@/lib/schemas";

const hash = "a".repeat(64);

describe("schemas", () => {
  it("accepts account responses with string balances", () => {
    const account = accountSchema.parse({
      account_id: hash,
      raw_address: `8:${hash}`,
      user_friendly_address: "EXaddress",
      status: "active",
      nonce: 1,
      balances: [{ asset_id: 0, amount: "1000000000000" }],
      code_hash: hash,
      data_hash: hash,
      storage_root: hash,
      last_lt: 3,
    });

    expect(account.balances[0].amount).toBe("1000000000000");
  });

  it("accepts flattened transaction detail responses", () => {
    const detail = transactionDetailSchema.parse({
      block_height: 1,
      tx_index: 0,
      timestamp: 10,
      block_hash: hash,
      tx_hash: hash,
      kind: "transfer",
      direction: "out",
      participants: [],
      asset_id: 0,
      amount: "5",
      status: "applied",
      gas_charged: "2",
      reason: null,
      withdrawal_id: null,
      chain_id: "entropis-testnet",
      nonce: 0,
      gas_limit: 1000,
      max_gas_price: "1",
      tx_root: hash,
      receipt_root: hash,
      withdrawal_root: hash,
      data_hash: hash,
      state_root: hash,
      raw_transaction: {},
      raw_receipt: null,
    });

    expect(detail.kind).toBe("transfer");
  });
});
