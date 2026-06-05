import { describe, expect, it } from "vitest";
import {
  accountSchema,
  blockSummarySchema,
  depositStatusSchema,
  transactionDetailSchema,
  withdrawalStatusSchema,
} from "@/lib/schemas";

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

  it("accepts block, deposit, and withdrawal explorer responses", () => {
    expect(
      blockSummarySchema.parse({
        height: 7,
        block_hash: hash,
        timestamp: 10,
        tx_count: 2,
        deposit_count: 1,
        withdrawal_count: 1,
        state_root: hash,
        data_hash: hash,
        withdrawal_root: hash,
      }).height,
    ).toBe(7);

    expect(
      depositStatusSchema.parse({
        status: "included",
        block_height: 7,
        tx_hash: hash,
        deposit: {
          deposit_id: hash,
          asset_id: 0,
          recipient: hash,
          amount: "100",
        },
      }).deposit.amount,
    ).toBe("100");

    expect(
      withdrawalStatusSchema.parse({
        status: "finalized",
        block_height: 7,
        batch_no: 8,
        proof_available: true,
        withdrawal_root: hash,
        finalization: null,
        leaf: {
          withdrawal_id: hash,
          asset_id: 0,
          amount: "100",
          l2_sender: hash,
          l1_recipient: "EQ...",
        },
      }).proof_available,
    ).toBe(true);
  });
});
