# TON_L2_SKILLS

Bootstrap date: 2026-06-04

This is the repository-local evolving knowledge base for TON L1 + TON-anchored L2 rollup work. Update it when implementation choices change or when newer primary docs supersede the current assumptions.

## Source Priority

- TON Docs: https://docs.ton.org/
- Tolk Docs: https://docs.ton.org/blockchain-basics/tolk/overview
- Acton Docs: https://ton-blockchain.github.io/acton/docs/welcome
- Jetton Docs / TEP-74 interface: https://docs.ton.org/blockchain-basics/standard/tokens/jettons/api
- Rollup design references: https://ethereum.org/developers/docs/scaling/optimistic-rollups/ and https://docs.optimism.io/op-stack/protocol/overview

## Structured Skills

```text
TON_L2_SKILLS = {
  ton_l1: [
    "TON is a multi-blockchain system: one masterchain anchors protocol parameters, validators, workchains, shardchain block hashes, and global finality context.",
    "The basic workchain is workchain 0. It hosts normal TON accounts, wallets, and smart contracts.",
    "Workchains may be split into many shardchains. Account-to-account interaction is message-based, so contracts must be designed as actors.",
    "Validators are selected by PoS and run BFT-style consensus for shardchain/masterchain block production.",
    "For this L2, TON L1 is the settlement and custody layer: RollupRoot stores commitments/finality, AssetVault locks and releases assets."
  ],
  tvm_execution: [
    "TVM is stack-based and deterministic for the same input message, prior state, code, and config context.",
    "TVM data is immutable cell DAG data; slices read cells and builders create cells.",
    "Every instruction consumes gas; gas exhaustion aborts execution.",
    "Persistent smart-contract data is cell-based account state, commonly exposed in Tolk via typed storage load/save helpers.",
    "L2 deterministic execution must mirror TVM determinism: canonical input order, no randomness, no host-clock state transitions."
  ],
  tolk_contracts: [
    "Tolk is the actively supported TON smart-contract language and compiles to TVM.",
    "Prefer typed structs, fixed-width serialized fields, union message dispatch, typed Cell<T> refs, lazy loading, and typed getters.",
    "Use opcode-prefixed structs for inbound messages and a union listed in contract incomingMessages.",
    "Use onInternalMessage for normal wallet/contract interaction and onBouncedMessage when outbound sends can bounce and state must be tracked.",
    "Unknown-message policy should be explicit: ignore empty top-ups, throw a known error for non-empty unknown bodies."
  ],
  acton_toolchain: [
    "Acton is the unified TON smart-contract CLI around Tolk: scaffold, build, test, script, wallet, verify, lint, format, and low-level tooling.",
    "Local first checks: acton --version, acton doctor, inspect Acton.toml, then acton build / acton test / acton check / acton fmt --check.",
    "On Windows, official Acton docs require WSL Ubuntu 22+; native Windows is unsupported.",
    "Acton contract test getters must use names beginning with `test`; otherwise a `.test.tolk` file may compile while reporting zero executed tests.",
    "Do not broadcast mainnet scripts before build, test, local emulation, and testnet validation.",
    "Regenerate wrappers when ABI changes; do not hand-edit generated wrappers unless unavoidable."
  ],
  jettons: [
    "Jettons are TON fungible tokens standardized by TEP-74.",
    "The Jetton architecture is distributed: a master/minter contract stores supply and metadata; per-owner wallet contracts hold balances and execute transfers.",
    "Bridge deposits must verify the sending jetton wallet and decode transfer_notification payloads instead of treating Jettons like account balances on one contract.",
    "Important opcodes include transfer 0x0f8a7ea5, transfer_notification 0x7362d09c, burn 0x595f07bc, and excesses 0xd53276db.",
    "TEP-74 transfer_notification contains query_id, amount, sender, and forward_payload as Either Cell ^Cell; Entropis accepts a ref cell payload containing exactly one uint256 L2 recipient.",
    "AssetVault stores an admin-managed asset registry: asset id, Jetton master, vault-owned Jetton wallet, decimals, and wallet-hash reverse index.",
    "Always respect token decimals and wallet discovery; do not assume 9 decimals or a globally shared balance contract."
  ],
  message_model: [
    "TON contracts interact mainly via internal async messages. A transaction consumes one inbound message, updates one account, and may emit zero or more outbound messages.",
    "Internal messages carry src, dest, value, bounce flags, logical time, timestamp, optional StateInit, and body.",
    "Outbound messages use send modes that define fee payment, remaining-balance behavior, ignore-errors behavior, and bounce-on-action-failure behavior.",
    "Deposits are L1 -> L2 observations: AssetVault emits canonical deposit logs, the indexer feeds them to the sequencer, and the L2 credits the recipient.",
    "Toncenter v3 `/messages` can filter log messages with `destination=null`, vault `source`, `opcode`, `start_lt`, `limit`, and `sort=asc`; use per-source cursors and fail closed on malformed expected logs.",
    "Withdrawals are L2 -> L1 claims: L2 creates withdrawal leaves, RollupRoot verifies inclusion after finalized commitment, then tells AssetVault to release."
  ],
  cell_boc_system: [
    "A TON cell holds up to 1023 bits and up to 4 refs; cells form a DAG and circular refs are impossible.",
    "BoC serializes a forest/DAG of cells for storage, network transport, messages, code, data, and proofs.",
    "Merkle proof exotic cells commit to a referenced subtree while allowing verification against a root hash without the full tree.",
    "Merkle update exotic cells represent old/new subtree commitments and are relevant for future fraud/challenge paths.",
    "L2 state roots should be canonical hashes over sparse Merkle/cell-compatible account leaves, with explicit domain separation.",
    "Consensus-critical L2 hashes use Entropis consensus encoding v1 (`EL2C`, version byte, type tag, big-endian integers, length-prefixed strings/bytes, explicit option tags), not JSON serialization."
  ],
  l2_rollup_design: [
    "Optimistic rollups move execution and state storage off-chain, batch transactions, and commit roots/data commitments to L1.",
    "Security requires data availability sufficient for independent re-execution and future fraud proofs.",
    "For TON, L1 stores compact batch commitments: prevStateRoot, stateRoot, txRoot, receiptRoot, withdrawalRoot, dataHash, timestamp, finalized flag.",
    "Entropis DA stores canonical BatchData bytes, not JSON; relayer must verify retrievability and hash/block binding before asking a signer to submit CommitBatch.",
    "DA can start as external/Ton Storage-backed data referenced by dataHash, but production fraud proofs require reliable retrievability and challenge rules.",
    "The MVP is optimistic/trusted-sequencer until challenge verification is implemented."
  ],
  sequencer_logic: [
    "Sequencer owns ordering in the MVP: ingest deposits, validate L2 txs, sort/canonicalize, execute deterministically, produce receipts and state root.",
    "Batch building should include previous state root, new state root, tx root, receipt root, withdrawal root, DA hash, and monotonic batch number.",
    "Deterministic batch building is isolated from mempool, execution, storage, Redis, network, and wall-clock reads; it consumes ordered txs, receipts, withdrawals, previous header/root, final state root, and an explicit timestamp.",
    "RollupRoot batch numbers are one-based while L2 block heights are zero-based; block height 0 must be committed as batchNo 1.",
    "Mempool admission must reject malformed signatures, bad nonces, insufficient balances, unsupported call types, non-canonical encodings, oversized payloads, bad gas/fee policy, per-account floods, global queue floods, and rate-limit abuse.",
    "Rust executor must isolate deterministic transition logic from networking, wall clock, persistence, and RPC/indexer effects.",
    "Executor gas is versioned config: applied fees are gas_used * max_gas_price in ENT asset id 0; authenticated rejected execution advances nonce and charges only rejected_execution_gas when possible.",
    "CallContract uses a TvmExecutionAdapter boundary: single-root BoC input, explicit deterministic context, contract-local state delta, bounded internal messages/body sizes, gas_used validation, and noop fail-closed behavior until the real TON TVM emulator is wired.",
    "Future decentralization path: multiple sequencers, proposer bonds, forced inclusion, and observer/challenger nodes."
  ],
  bridge_design: [
    "TON deposits: user sends TON to AssetVault with DepositTon body; vault records locked amount and emits DepositRecorded external log.",
    "Jetton deposits: user sends Jettons through their Jetton wallet with forward payload containing L2 recipient; vault handles transfer_notification only from registered vault-owned Jetton wallets.",
    "Jetton releases: AssetVault sends TEP-74 transfer to the registered vault-owned Jetton wallet, uses contract.getAddress() as response_destination, tracks pending query ids, clears them on excesses, and records wallet bounces as retryable failures.",
    "L2 credits only indexer-confirmed vault events with canonical deposit ids and replay protection; the Rust indexer accepts only configured L1_DEPOSIT_ASSET_IDS.",
    "The L1 batch relayer persists pending/submitted/confirmed/failed status, uses bounded retries, submits signed external BoCs through Toncenter v3 `/message`, and observes confirmation through `/transactionsByMessage`.",
    "The node should not hold raw TON wallet credentials for relaying; use a signer boundary and verify the returned signer address matches RollupRoot.sequencer before broadcasting.",
    "Withdrawals: L2 creates withdrawal leaves; after batch finalization, user submits a ReleaseAuthorized leaf cell + compact Merkle proof to RollupRoot; root sends ReleaseAuthorized to AssetVault.",
    "The committed withdrawalRoot is the Merkle root of ReleaseAuthorized cell representation hashes; withdrawal tree node hashes are representation hashes of a compact cell containing left uint256 then right uint256.",
    "Root-to-vault release bounces are stored in RollupRoot.failedWithdrawals without deleting claimedWithdrawals; RetryWithdrawal is permissionless and resends only stored release fields.",
    "Vault-to-recipient TON release bounces are stored in AssetVault.releaseFailures, re-credit lockedTon, and can be retried permissionlessly through RetryRelease.",
    "Unsupported release assets remain visible as failures and are not retryable until the asset is registered or a future wrapped-gas flow is implemented."
  ],
  infrastructure: [
    "Entropis testnet uses chain id entropis-testnet and ENT as the L2-native gas token symbol.",
    "ENT is L2-native first in the MVP: decimals=9, logo at assets/entropis.png, faucet-only testnet supply, no L1 Jetton minter/wallet until bridge/indexer hardening is stable.",
    "Testnet node config must refuse TON mainnet endpoints; Toncenter v3 testnet is https://testnet.toncenter.com/api/v3.",
    "Toncenter API keys are sent through X-API-Key; TonAPI keys use Authorization: Bearer <token> against https://testnet.tonapi.io.",
    "Runtime secrets belong in .env.local or environment variables only; tracked files may include .env.example placeholders but never real keys.",
    "Postgres persists L2 blocks, transactions, deposits, withdrawals, L1 cursors, and ENT faucet grants; Redis owns public mempool replay, nonce locks, per-account queue counters, rate-limit counters, and sequencer leader-lock responsibilities."
  ],
  security_patterns: [
    "Use explicit admin/sequencer authorization and pausability for emergency response.",
    "Track claimed withdrawals before sending release messages to prevent reentrancy-style double claims in async flow.",
    "Bind ClaimWithdrawal.withdrawalId to the decoded ReleaseAuthorized.withdrawalId before marking claims or sending vault release messages.",
    "Do not clear claimedWithdrawals on root-to-vault bounce; store a failed release and retry from stored fields to avoid reopening proof claims.",
    "Recipient bounce handling must verify senderAddress equals the original recipient and must never accept caller-supplied retry amount or recipient.",
    "Use domain-separated hashes for deposits, transactions, receipts, withdrawals, and blocks.",
    "Never rely on unordered map iteration for root computation.",
    "Keep challengeWindowSec and finalization logic conservative until fraud proofs are implemented."
  ],
  common_bugs: [
    "Staging .env, wallet files, mnemonics, API keys, node databases, build artifacts, target, node_modules, or Acton caches.",
    "Treating Jetton master as the holder of user balances instead of using owner-specific Jetton wallets.",
    "Using non-canonical serialization or host-dependent ordering in Rust state roots.",
    "Allowing withdrawal claims before commitment finalization.",
    "Failing to handle bounced L1 release messages or failed outbound send modes."
  ],
  best_practices: [
    "Design storage and message schemas before behavior; keep field order and integer widths explicit.",
    "Prefer Tolk typed auto-serialization and union dispatch over raw manual slice parsing unless a boundary requires it.",
    "Run cargo test for Rust changes; run acton build/test/check/fmt when Acton is available.",
    "Keep source, tests, lockfiles, and docs in Git; ignore generated artifacts, caches, local databases, and secret material.",
    "When building new L2 features, document architecture, message flow, state model, bridge impact, Acton commands, risks, and limitations."
  ]
}
```

## Open Knowledge Gaps

- Exact fraud-proof VM design for replaying TON-compatible L2 transitions on L1 is not implemented.
- Data availability backend is not finalized: TON Storage vs external DA vs hybrid.
- Jetton release path is v2 work: MVP records failure for non-TON asset releases.
- Acton cannot be run from native PowerShell in this environment; use WSL or Docker for contract checks.
