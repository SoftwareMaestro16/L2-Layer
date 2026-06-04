# Testnet Batch Commit E2E

This runbook proves the live L1 settlement path:

```text
producer -> block + canonical DA -> l1_batch_commits pending
  -> relayer DA verify -> signer CommitBatch BoC
  -> Toncenter v3 /message -> RollupRoot.commitment(batchNo)
  -> l1_batch_commits confirmed
```

The node never stores raw wallet credentials. The relayer asks an external signer
for a typed `CommitBatch` message and refuses to broadcast when DA verification,
signer identity, or provider submission fails.

## Prerequisites

- `deployments/testnet/entropis.json` has a verified `RollupRoot` address.
- `RollupRoot.sequencer` equals `L1_SEQUENCER_SENDER_ADDRESS`.
- `L2_RUNTIME_MODE=testnet-prototype`.
- `L1_BATCH_RELAYER_ENABLED=true`.
- `L1_ROLLUP_ROOT_ADDRESS` matches the verified registry root address.
- `L1_COMMIT_SIGNER_ENDPOINT` points to the operator signer service.
- `L1_COMMIT_SIGNER_TOKEN` is present only in `.env.local` or process env.
- `TONCENTER_V3_BASE_URL=https://testnet.toncenter.com/api/v3`.
- `L1_BATCH_RELAYER_MAX_ATTEMPTS` and retry backoff are non-zero.

## Produce A Pending Batch

Create at least one L2 transaction or indexed deposit, then wait for the producer.
The producer writes canonical batch bytes through the DA store before saving the
block and pending L1 relay row.

Check operator status:

```powershell
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/batch-commits
```

Expected before relay submission:

- `batch_commits[].status = "pending"`.
- `batch_commits[].batch_no = block_height + 1`.
- `batch_commits[].block_hash` matches `GET /v1/block/:height`.
- `message_hash` and `message_hash_norm` are `null`.

## Verify DA Before Signing

For local Postgres DA, the relayer calls the same `DaVerifier` used in tests. The
payload must match:

- `block_height`.
- `block_hash`.
- `data_hash`.
- configured `DA_MAX_PAYLOAD_BYTES`.

If the payload is missing, partial, corrupted, oversized, or bound to another
block, the relayer stores `status = "failed"` and `last_error = "batch data
unavailable"`. It must not call the signer and must not call Toncenter.

## Sign And Submit CommitBatch

The relayer builds a typed sign request:

- `rollup_root_address = L1_ROLLUP_ROOT_ADDRESS`.
- `sender_address = L1_SEQUENCER_SENDER_ADDRESS`.
- `msg_value_nanoton = L1_COMMIT_MSG_VALUE_NANOTON`.
- `commitment.batch_no = block_height + 1`.
- `roots_a = prevStateRoot, stateRoot, txRoot`.
- `roots_b = receiptRoot, withdrawalRoot, dataHash`.

The signer returns `{ boc_base64, signer_address }`. The relayer rejects the
response before broadcast when `signer_address` differs from
`L1_SEQUENCER_SENDER_ADDRESS` or the signed BoC is empty.

Toncenter v3 submission uses:

```text
POST /api/v3/message
{ "boc": "<signed external message BoC base64>" }
```

On success, the node stores `message_hash` and `message_hash_norm` and marks the
row `submitted`.

## Confirm On TON

The relayer polls Toncenter v3:

```text
GET /api/v3/transactionsByMessage?msg_hash=<message_hash_norm>&direction=in&limit=1
```

After Toncenter returns an inbound transaction, the node marks the row
`confirmed`.

Check:

```powershell
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/batch-commits
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/metrics
```

Expected:

- `batch_commits[].status = "confirmed"`.
- `batch_commits[].message_hash_norm` is set.
- `node.relayer.confirmed` increases.
- `node.relayer.failed` does not increase for the happy path.

## Verify RollupRoot Getter

Read `RollupRoot.commitment(batchNo)` through Acton or Toncenter get-method
support and compare it with the L2 block header:

- `prevStateRoot` equals `block.header.prev_state_root`.
- `stateRoot` equals `block.header.state_root`.
- `txRoot` equals `block.header.tx_root`.
- `receiptRoot` equals `block.header.receipt_root`.
- `withdrawalRoot` equals `block.header.withdrawal_root`.
- `dataHash` equals `block.header.data_hash`.
- `committedAt` is non-zero.
- `finalized` remains false before the challenge window finalizer runs.

The getter readback is public metadata. Do not write signer tokens, API keys,
signed BoCs, mnemonics, wallet seeds, database URLs, or Redis URLs into docs or
deployment manifests.

## Failure And Retry Checks

Use:

```powershell
curl -H "Authorization: Bearer $env:L2_ADMIN_TOKEN" `
  http://127.0.0.1:8080/v1/operator/failures
```

Expected safe failures:

- `batch data unavailable`: fix DA storage and let bounded retries continue.
- `commit signer failed`: check signer health, typed allowlist, token, and root
  address.
- `commit signer address mismatch`: signer role does not match root sequencer.
- `signed boc is empty`: signer returned an invalid envelope.
- `ton provider send failed`: check Toncenter availability and testnet endpoint.
- `l2 block missing` or `l2 block hash mismatch`: stop relayer and audit storage.

Retries are bounded by `L1_BATCH_RELAYER_MAX_ATTEMPTS`; after the attempt cap,
the relayer stops selecting that failed row. Do not manually reset attempts until
the root cause is fixed.

## Negative Tests

The Rust test suite covers the operator and relayer safety path:

```powershell
cargo test -p l2-node operator_batch_commits --lib
cargo test -p l2-node relayer --lib
```

These tests check unauthorized operator access, DA missing/corrupt before
signing, signer address mismatch before provider send, duplicate submitted batch
confirmation without re-send, and bounded provider retry attempts.
