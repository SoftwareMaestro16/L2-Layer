import { z } from "zod";

export const hashSchema = z.string().regex(/^[0-9a-fA-F]{64}$/);
const amountSchema = z.string().regex(/^\d+$/);

export const balanceSchema = z.object({
  asset_id: z.number().int().nonnegative(),
  amount: amountSchema,
});

export const accountSchema = z.object({
  account_id: hashSchema,
  raw_address: z.string(),
  user_friendly_address: z.string(),
  status: z.string(),
  nonce: z.number().int().nonnegative(),
  balances: z.array(balanceSchema),
  code_hash: hashSchema,
  data_hash: hashSchema,
  storage_root: hashSchema,
  last_lt: z.number().int().nonnegative(),
});

export const participantSchema = z.object({
  role: z.string(),
  account_id: hashSchema,
  raw_address: z.string(),
  user_friendly_address: z.string(),
});

export const transactionSummarySchema = z.object({
  block_height: z.number().int().nonnegative(),
  tx_index: z.number().int().nonnegative(),
  timestamp: z.number().int().nonnegative(),
  block_hash: hashSchema,
  tx_hash: hashSchema,
  kind: z.string(),
  direction: z.string(),
  participants: z.array(participantSchema),
  asset_id: z.number().int().nonnegative().nullable(),
  amount: amountSchema.nullable(),
  status: z.string(),
  gas_charged: amountSchema.nullable(),
  reason: z.string().nullable(),
  withdrawal_id: hashSchema.nullable(),
});

export const transactionCursorSchema = z.object({
  before_height: z.number().int().nonnegative(),
  before_index: z.number().int().nonnegative(),
});

export const accountTransactionsSchema = z.object({
  items: z.array(transactionSummarySchema),
  next_cursor: transactionCursorSchema.nullable(),
});

export const transactionDetailSchema = transactionSummarySchema.extend({
  chain_id: z.string(),
  nonce: z.number().int().nonnegative(),
  gas_limit: z.number().int().nonnegative(),
  max_gas_price: amountSchema,
  tx_root: hashSchema,
  receipt_root: hashSchema,
  withdrawal_root: hashSchema,
  data_hash: hashSchema,
  state_root: hashSchema,
  raw_transaction: z.unknown(),
  raw_receipt: z.unknown().nullable(),
});

export const healthSchema = z.object({
  status: z.string(),
  service: z.string().optional(),
});

export type Account = z.infer<typeof accountSchema>;
export type AccountTransactions = z.infer<typeof accountTransactionsSchema>;
export type TransactionDetail = z.infer<typeof transactionDetailSchema>;
export type TransactionSummary = z.infer<typeof transactionSummarySchema>;
