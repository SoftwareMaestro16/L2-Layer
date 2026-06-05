import { describe, expect, it } from "vitest";
import { explorerSummarySchema, sourceSchema, transactionDetailSchema } from "./schemas";

const hash = "a".repeat(64);

describe("schemas", () => {
  it("parses explorer summary stats", () => {
    expect(
      explorerSummarySchema.parse({
        latest_block: null,
        latest_batch_commit: null,
        latest_confirmed_commit: null,
        latest_finalization: null,
        latest_finalized_batch: null,
        block_count: 1,
        transaction_count: 2,
        deposit_count: 3,
        withdrawal_count: 4,
        live_account_count: 5,
        live_ent_supply: "1000"
      }).transaction_count
    ).toBe(2);
  });

  it("keeps verifier source states explicit", () => {
    expect(sourceSchema.parse({ status: "pending", code_hash: hash, submission_id: hash, files: [] }).status).toBe("pending");
  });

  it("parses transaction flow", () => {
    expect(
      transactionDetailSchema.parse({
        block_height: 1,
        tx_index: 0,
        timestamp: 1,
        block_hash: hash,
        tx_hash: hash,
        kind: "transfer",
        interface: null,
        interface_label: null,
        operation: null,
        direction: "out",
        participants: [],
        asset_id: 0,
        amount: "1",
        status: "applied",
        gas_charged: "1",
        reason: null,
        withdrawal_id: null,
        event_count: 0,
        flow: [{ id: "receipt", label: "Receipt", role: "receipt", account_id: null, raw_address: null, user_friendly_address: null, asset_id: null, amount: null, gas_charged: "1", status: "applied", reason: null, details: {} }],
        chain_id: "entropis-testnet",
        nonce: 0,
        gas_limit: 1,
        max_gas_price: "1",
        tx_root: hash,
        receipt_root: hash,
        withdrawal_root: hash,
        data_hash: hash,
        state_root: hash,
        raw_transaction: {},
        raw_receipt: null
      }).flow[0].role
    ).toBe("receipt");
  });
});
