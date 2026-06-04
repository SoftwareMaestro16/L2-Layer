# Testnet Signer Service

The signer is a separate process from `l2-node`. The node never stores wallet
mnemonics, private keys, plaintext wallet files, or raw Acton wallet exports.

## Roles

- `deployer_admin`: deploys `RollupRoot`, `AssetVault`, and future deployment
  scripts.
- `sequencer`: signs `CommitBatch` and `FinalizeBatch` messages for the
  relayer/finalizer.
- `vault_admin`: registers assets and performs vault-admin operations.
- `operator`: reserved for future claim/retry and emergency runbooks.

Use separate wallets for deployment/admin and sequencer relaying. A compromised
batch signer must not be able to rotate vault assets or redeploy contracts.

## Local Wallet Policy

Create testnet wallets in WSL/Linux with Acton secure storage. Do not commit
`wallets.toml`, mnemonic files, wallet exports, or copied seed phrases.

```powershell
wsl acton wallet new --name entropis-testnet-deployer --global --version v5r1 --secure true
wsl acton wallet new --name entropis-testnet-sequencer --global --version v5r1 --secure true
wsl acton wallet list --balance
```

Use `.env.local` for labels and public sender addresses only:

```text
L1_DEPLOYER_WALLET_LABEL=entropis-testnet-deployer
L1_SEQUENCER_WALLET_LABEL=entropis-testnet-sequencer
L1_SEQUENCER_SENDER_ADDRESS=<sequencer wallet address>
```

`acton wallet export-mnemonic` is an interactive escape hatch for migration only.
Do not paste its output into docs, logs, CI variables, or GitHub issues.

## HTTP Contract

The signer exposes typed endpoints:

- `POST /sign-commit`: MVP commit signer path.
- `POST /sign-finalize`: MVP finalization signer path.
- `POST /sign`: commit-compatible typed path kept for local dry-run clients.

All requests require:

```text
Authorization: Bearer <L2_SIGNER_TOKEN>
```

Commit request shape:

```json
{
  "request_id": "commit-batch-1-<block-hash>",
  "role": "sequencer",
  "valid_until": 1790000000,
  "action": "commit_batch",
  "payload": {
    "rollup_root_address": "<RollupRoot address>",
    "sender_address": "<sequencer wallet address>",
    "msg_value_nanoton": 100000000,
    "commitment": {
      "batch_no": 1,
      "block_height": 0,
      "block_hash": "<hex hash>",
      "roots_a": {
        "prev_state_root": "<hex hash>",
        "state_root": "<hex hash>",
        "tx_root": "<hex hash>"
      },
      "roots_b": {
        "receipt_root": "<hex hash>",
        "withdrawal_root": "<hex hash>",
        "data_hash": "<hex hash>"
      }
    }
  }
}
```

Response shape:

```json
{
  "request_id": "commit-batch-1-<block-hash>",
  "action": "commit_batch",
  "signer_address": "<sequencer wallet address>",
  "boc_base64": "<signed external message BoC>",
  "valid_until": 1790000000
}
```

Finalize request shape:

```json
{
  "request_id": "finalize-batch-1",
  "role": "sequencer",
  "valid_until": 1790000000,
  "action": "finalize_batch",
  "payload": {
    "rollup_root_address": "<RollupRoot address>",
    "sender_address": "<sequencer wallet address>",
    "batch_no": 1,
    "msg_value_nanoton": 100000000
  }
}
```

`POST /sign-finalize` rejects any action other than `finalize_batch`.
`POST /sign-commit` and `POST /sign` reject `finalize_batch`.

The node rejects signer responses before Toncenter broadcast when:

- `signer_address` differs from `L1_SEQUENCER_SENDER_ADDRESS`.
- `valid_until` is expired.
- `boc_base64` is empty, malformed, or oversized.
- the response request id or action does not match the request.
- the request `rollup_root_address` differs from
  `L2_SIGNER_ROLLUP_ROOT_ADDRESS` when that allowlist is configured.

## Running The Service

`l2-signer` validates typed requests and delegates signing to an external command.
The command receives the typed JSON request on stdin and must print this JSON to
stdout:

```json
{
  "boc_base64": "<signed external message BoC>",
  "signer_address": "<configured signer address>",
  "valid_until": 1790000000
}
```

Environment:

```text
L2_SIGNER_ADDR=127.0.0.1:8800
L2_SIGNER_TOKEN=<local bearer token>
L2_SIGNER_ADDRESS=<sequencer wallet address>
L2_SIGNER_ROLLUP_ROOT_ADDRESS=<RollupRoot address>
L2_SIGNER_ROLE=sequencer
L2_SIGNER_COMMAND=<local signer command path>
L2_SIGNER_COMMAND_TIMEOUT_MS=5000
L2_SIGNER_MAX_BODY_BYTES=16384
L2_SIGNER_RATE_LIMIT_PER_MINUTE=60
```

Start it locally:

```powershell
cargo run -p l2-node --bin l2-signer
```

The external command is the only component allowed to touch wallet tooling,
keyring access, or future HSM APIs. It must build/sign only the typed action it
receives; do not implement a raw-payload signing mode.

## Dry Run Without Broadcast

For a no-broadcast test, run `l2-signer` against a local signer command and post a
single `commit_batch` request to `http://127.0.0.1:8800/sign-commit`, then a
`finalize_batch` request to `http://127.0.0.1:8800/sign-finalize` for an already
committed batch. Confirm the response has the expected `signer_address`, a future
`valid_until`, and a BoC that passes local validation. Do not send the returned
BoC to Toncenter until the testnet deploy addresses and signer command have been
reviewed.

## Safe Error Codes

The service returns static error codes such as `unauthorized`, `rate_limited`,
`unsupported_action`, `expired_request`, `signer_role_mismatch`,
`rollup_root_mismatch`, `signer_address_mismatch`, `malformed_boc`, and
`signer_backend_timeout`.

Logs and persistent node state must never include bearer tokens, mnemonics,
wallet exports, raw provider responses, or raw signed BoCs.
