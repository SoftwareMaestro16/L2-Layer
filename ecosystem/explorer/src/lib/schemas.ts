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

export const readyzSchema = z.object({
  status: z.string(),
  components: z.record(
    z.string(),
    z.object({
      status: z.string(),
      reason: z.string().nullable().optional(),
    }).passthrough(),
  ).optional(),
}).passthrough();

export const blockSummarySchema = z.object({
  height: z.number().int().nonnegative(),
  block_hash: hashSchema,
  timestamp: z.number().int().nonnegative(),
  tx_count: z.number().int().nonnegative(),
  deposit_count: z.number().int().nonnegative(),
  withdrawal_count: z.number().int().nonnegative(),
  state_root: hashSchema,
  data_hash: hashSchema,
  withdrawal_root: hashSchema,
});

export const batchStatusSchema = z.object({
  batch_no: z.number().int().nonnegative(),
  block_height: z.number().int().nonnegative(),
  block_hash: hashSchema,
  status: z.string(),
  message_hash_norm: hashSchema.nullable(),
});

export const finalizationStatusSchema = z.object({
  batch_no: z.number().int().nonnegative(),
  block_height: z.number().int().nonnegative(),
  status: z.string(),
  finalize_after_unix: z.number().int().nonnegative(),
  message_hash_norm: hashSchema.nullable(),
});

export const explorerSummarySchema = z.object({
  latest_block: blockSummarySchema.nullable(),
  latest_batch_commit: batchStatusSchema.nullable(),
  latest_confirmed_commit: batchStatusSchema.nullable().optional(),
  latest_finalization: finalizationStatusSchema.nullable().optional(),
  latest_finalized_batch: finalizationStatusSchema.nullable(),
});

export const pagedBlocksSchema = z.object({
  items: z.array(blockSummarySchema),
  next_before_height: z.number().int().nonnegative().nullable(),
});

export const depositStatusSchema = z.object({
  status: z.string(),
  block_height: z.number().int().nonnegative(),
  tx_hash: hashSchema,
  deposit: z.object({
    deposit_id: hashSchema,
    asset_id: z.number().int().nonnegative(),
    recipient: hashSchema,
    amount: amountSchema,
  }),
});

export const pagedDepositsSchema = z.object({
  items: z.array(depositStatusSchema),
  next_before_height: z.number().int().nonnegative().nullable(),
});

export const withdrawalStatusSchema = z.object({
  status: z.string(),
  block_height: z.number().int().nonnegative(),
  batch_no: z.number().int().nonnegative(),
  proof_available: z.boolean(),
  withdrawal_root: hashSchema,
  finalization: finalizationStatusSchema.nullable(),
  leaf: z.object({
    withdrawal_id: hashSchema,
    asset_id: z.number().int().nonnegative(),
    amount: amountSchema,
    l2_sender: hashSchema,
    l1_recipient: z.string(),
  }).passthrough(),
});

export const contractStateSchema = z.object({
  account_id: hashSchema.optional(),
  contract: hashSchema.optional(),
  raw_address: z.string().optional(),
  user_friendly_address: z.string().optional(),
  code_hash: hashSchema,
  data_hash: hashSchema,
  storage_root: hashSchema,
  code_boc_base64: z.string().optional(),
  data_boc_base64: z.string().optional(),
  last_lt: z.number().int().nonnegative().optional(),
}).passthrough();

export const rawJsonSchema = z.unknown();

export type Account = z.infer<typeof accountSchema>;
export type AccountTransactions = z.infer<typeof accountTransactionsSchema>;
export type BatchStatus = z.infer<typeof batchStatusSchema>;
export type BlockSummary = z.infer<typeof blockSummarySchema>;
export type ContractState = z.infer<typeof contractStateSchema>;
export type DepositStatus = z.infer<typeof depositStatusSchema>;
export type ExplorerSummary = z.infer<typeof explorerSummarySchema>;
export type FinalizationStatus = z.infer<typeof finalizationStatusSchema>;
export type PagedBlocks = z.infer<typeof pagedBlocksSchema>;
export type PagedDeposits = z.infer<typeof pagedDepositsSchema>;
export type TransactionDetail = z.infer<typeof transactionDetailSchema>;
export type TransactionSummary = z.infer<typeof transactionSummarySchema>;
export type WithdrawalStatus = z.infer<typeof withdrawalStatusSchema>;
