import { z } from "zod";

const hashSchema = z.string().regex(/^[a-fA-F0-9]{64}$/u);
const amountSchema = z.string().regex(/^\d+$/u);

export const participantSchema = z.object({
  role: z.string(),
  account_id: hashSchema,
  raw_address: z.string(),
  user_friendly_address: z.string()
});

export const blockSummarySchema = z.object({
  height: z.number(),
  block_hash: hashSchema,
  timestamp: z.number(),
  tx_count: z.number(),
  deposit_count: z.number(),
  withdrawal_count: z.number(),
  state_root: hashSchema,
  data_hash: hashSchema,
  withdrawal_root: hashSchema
});

export const explorerSummarySchema = z.object({
  latest_block: blockSummarySchema.nullable(),
  latest_batch_commit: z.object({ batch_no: z.number(), status: z.string() }).passthrough().nullable(),
  latest_confirmed_commit: z.object({ batch_no: z.number(), status: z.string() }).passthrough().nullable(),
  latest_finalization: z.object({ batch_no: z.number(), status: z.string() }).passthrough().nullable(),
  latest_finalized_batch: z.object({ batch_no: z.number(), status: z.string() }).passthrough().nullable(),
  block_count: z.number(),
  transaction_count: z.number(),
  deposit_count: z.number(),
  withdrawal_count: z.number(),
  live_account_count: z.number(),
  live_ent_supply: amountSchema
});

export const accountSchema = z.object({
  account_id: hashSchema,
  raw_address: z.string(),
  user_friendly_address: z.string(),
  status: z.string(),
  nonce: z.number(),
  balances: z.array(z.object({ asset_id: z.number(), amount: amountSchema })),
  code_hash: hashSchema,
  data_hash: hashSchema,
  storage_root: hashSchema,
  interfaces: z.array(z.object({ id: z.string(), label: z.string() })),
  last_lt: z.number()
});

export const transactionSummarySchema = z.object({
  block_height: z.number(),
  tx_index: z.number(),
  timestamp: z.number(),
  block_hash: hashSchema,
  tx_hash: hashSchema,
  kind: z.string(),
  interface: z.string().nullable(),
  interface_label: z.string().nullable(),
  operation: z.string().nullable(),
  direction: z.string(),
  participants: z.array(participantSchema),
  asset_id: z.number().nullable(),
  amount: amountSchema.nullable(),
  status: z.string(),
  gas_charged: amountSchema.nullable(),
  reason: z.string().nullable(),
  withdrawal_id: hashSchema.nullable(),
  event_count: z.number()
});

export const transactionListSchema = z.object({
  items: z.array(transactionSummarySchema),
  next_cursor: z.object({ before_height: z.number(), before_index: z.number() }).nullable()
});

export const flowNodeSchema = z.object({
  id: z.string(),
  label: z.string(),
  role: z.string(),
  account_id: hashSchema.nullable(),
  raw_address: z.string().nullable(),
  user_friendly_address: z.string().nullable(),
  asset_id: z.number().nullable(),
  amount: amountSchema.nullable(),
  gas_charged: amountSchema.nullable(),
  status: z.string().nullable(),
  reason: z.string().nullable(),
  details: z.unknown()
});

export const transactionDetailSchema = transactionSummarySchema.extend({
  flow: z.array(flowNodeSchema),
  chain_id: z.string(),
  nonce: z.number(),
  gas_limit: z.number(),
  max_gas_price: amountSchema,
  tx_root: hashSchema,
  receipt_root: hashSchema,
  withdrawal_root: hashSchema,
  data_hash: hashSchema,
  state_root: hashSchema,
  raw_transaction: z.unknown(),
  raw_receipt: z.unknown().nullable()
});

export const assetsSchema = z.object({
  tokens: z.array(z.object({
    id: z.string(),
    asset_id: z.number(),
    symbol: z.string(),
    name: z.string(),
    decimals: z.number(),
    amount: amountSchema
  })),
  collectibles: z.array(z.object({ id: z.string(), name: z.string(), collection: z.string() }))
});

const sourceFileSchema = z.object({ path: z.string(), content: z.string() });

export const sourceSchema = z.object({
  status: z.enum(["not_found", "pending", "verified", "rejected"]),
  code_hash: hashSchema,
  submission_id: hashSchema.nullable(),
  files: z.array(sourceFileSchema)
});

export const codeSchema = z.object({
  account_id: hashSchema,
  bytecode: cellViewSchema(),
  raw_data: cellViewSchema(),
  source: sourceSchema
});

function cellViewSchema() {
  return z.object({
    hex: z.string(),
    base64: z.string(),
    hex_hash: hashSchema,
    root_hash: hashSchema,
    size_bytes: z.number(),
    cell_count: z.number(),
    cells: z.array(z.object({ index: z.number(), role: z.string(), hash: hashSchema, size_bytes: z.number() }))
  });
}

export type ExplorerSummary = z.infer<typeof explorerSummarySchema>;
export type ExplorerAccount = z.infer<typeof accountSchema>;
export type TransactionSummary = z.infer<typeof transactionSummarySchema>;
export type TransactionDetail = z.infer<typeof transactionDetailSchema>;
export type TransactionList = z.infer<typeof transactionListSchema>;
export type ExplorerAssets = z.infer<typeof assetsSchema>;
export type ExplorerCode = z.infer<typeof codeSchema>;
export type ExplorerSource = z.infer<typeof sourceSchema>;
export type FlowNode = z.infer<typeof flowNodeSchema>;
