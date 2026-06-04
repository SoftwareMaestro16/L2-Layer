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
    "Entropis runs contract checks through scripts/ci/acton_contract_checks.sh locally and in CI; CI pins ton-blockchain/setup-acton v1.0.0 by commit SHA and Acton 1.1.0.",
    "Docker fallback uses ghcr.io/ton-blockchain/acton:1.1.0 with isolated HOME/XDG_CACHE_HOME and no wallet or deployment secret mounts.",
    "Acton contract test getters must use names beginning with `test`; otherwise a `.test.tolk` file may compile while reporting zero executed tests.",
    "Acton deployment scripts can run in local emulation without `--net`; only explicit `--net testnet` aliases should broadcast for Entropis testnet deployment.",
    "Deployment output JSON belongs under ignored `build/` or `deployments/`; wallet overlays (`wallets.toml`, `global.wallets.toml`) must remain local-only.",
    "Do not broadcast mainnet scripts before build, test, local emulation, and testnet validation.",
    "Regenerate wrappers when ABI changes; do not hand-edit generated wrappers unless unavoidable."
  ],
  jettons: [
    "Jettons are TON fungible tokens standardized by TEP-74.",
    "The Jetton architecture is distributed: a master/minter contract stores supply and metadata; per-owner wallet contracts hold balances and execute transfers.",
    "Bridge deposits must verify the sending jetton wallet and decode transfer_notification payloads instead of treating Jettons like account balances on one contract.",
    "Important opcodes include transfer 0x0f8a7ea5, transfer_notification 0x7362d09c, burn 0x595f07bc, and excesses 0xd53276db.",
    "TEP-74 transfer_notification contains query_id, amount, sender, and forward_payload as Either Cell ^Cell; Entropis accepts canonical inline or ref branches only when the decoded payload is exactly one non-zero uint256 L2 recipient.",
    "AssetVault stores an admin-managed asset registry: asset id, Jetton master, vault-owned Jetton wallet, decimals, and wallet-hash reverse index.",
    "The SDK builds Jetton deposit transfers with destination=AssetVault, response_destination=user wallet, forward_ton_amount>0, and forward_payload as the canonical ref branch containing the L2 recipient cell.",
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
    "RollupRoot and AssetVault cannot mutually include each other's final address in both StateInit cells. Entropis deploys RollupRoot with a zero-address vault sentinel, deploys AssetVault with the computed root address, then links the root once through admin-only SetAssetVault.",
    "Entropis DA stores canonical BatchData bytes, not JSON; relayer must verify retrievability and hash/block binding before asking a signer to submit CommitBatch.",
    "Entropis public DA MVP uses Postgres as a mirror/cache plus a filesystem gateway path blocks/{height}/{blockHash}-{dataHash}.el2batch; the relayer verifies the public file when DA_PUBLIC_BACKEND=filesystem.",
    "Public DA retrieval endpoints serve application/octet-stream payload bytes by block height or by height+dataHash and must re-hash the body before returning it.",
    "DA can start as external/Ton Storage-backed data referenced by dataHash, but production fraud proofs require reliable retrievability and challenge rules.",
    "Challenge design separates DA retrieval, deterministic replay, witness/proof generation, and L1 challenge submission; missing DA is an availability challenge, not a state-transition proof.",
    "Entropis observer replay is currently off-chain: it accepts RollupRoot-shaped commitments, fetches canonical DA bytes by dataHash, replays from a trusted checkpoint, stores observer checkpoints, and reports missing_da/corrupt_da/invalid without submitting an L1 challenge.",
    "The MVP is optimistic/trusted-sequencer until challenge verification is implemented."
  ],
  sequencer_logic: [
    "Sequencer owns ordering in the MVP: ingest deposits, validate L2 txs, sort/canonicalize, execute deterministically, produce receipts and state root.",
    "Batch building should include previous state root, new state root, tx root, receipt root, withdrawal root, DA hash, and monotonic batch number.",
    "Deterministic batch building is isolated from mempool, execution, storage, Redis, network, and wall-clock reads; it consumes ordered txs, receipts, withdrawals, previous header/root, final state root, and an explicit timestamp.",
    "RollupRoot batch numbers are one-based while L2 block heights are zero-based; block height 0 must be committed as batchNo 1.",
    "Mempool admission must reject malformed signatures, bad nonces, insufficient balances, unsupported call types, non-canonical encodings, oversized payloads, bad gas/fee policy, per-account floods, global queue floods, and rate-limit abuse.",
    "Operator observability must split process liveness from dependency readiness; readiness responses expose safe component codes only and never include secret-bearing config.",
    "Explorer/operator endpoints should be read-only, bounded, and public-safe: show block/deposit/withdrawal inclusion and sanitized relay/finalization status, while failures and metrics remain admin-only.",
    "Observer/challenger nodes replay canonical DA bytes from a trusted state checkpoint, compare tx/receipt/withdrawal/state roots, and locate the first invalid transition before L1 challenge submission; they must not trust local sequencer block JSON as the commitment source.",
    "Rust executor must isolate deterministic transition logic from networking, wall clock, persistence, and RPC/indexer effects.",
    "Executor gas is versioned config: applied fees are gas_used * max_gas_price in ENT asset id 0; authenticated rejected execution advances nonce and charges only rejected_execution_gas when possible.",
    "CallContract uses a TvmExecutionAdapter boundary: single-root BoC input, explicit deterministic context, contract-local state delta, bounded internal messages/body sizes, gas_used validation, and noop fail-closed behavior until the real TON TVM emulator is wired.",
    "Future decentralization path: multiple sequencers, proposer bonds, forced inclusion, and observer/challenger nodes."
  ],
  bridge_design: [
    "TON deposits: user sends TON to AssetVault with DepositTon body; vault records locked amount and emits DepositRecorded external log.",
    "Native TON deposits must reject zero L2 recipients at the vault boundary, not only in the Rust indexer, because otherwise funds can be locked while the log is later discarded.",
    "The credited L2 deposit id should be derived from unique L1 event identity such as vault source, message hash, logical time, and event id; user-controlled query ids are not sufficient uniqueness for repeated real deposits.",
    "Jetton deposits: user sends Jettons through their Jetton wallet with forward payload containing L2 recipient; vault handles canonical transfer_notification bodies only from registered vault-owned Jetton wallets.",
    "Jetton releases: AssetVault sends TEP-74 transfer to the registered vault-owned Jetton wallet, uses contract.getAddress() as response_destination, derives pending query ids from transaction logical time, clears them on excesses, and records wallet bounces as retryable failures.",
    "L2 credits only indexer-confirmed vault events with canonical deposit ids and replay protection; the Rust indexer accepts only configured L1_DEPOSIT_ASSET_IDS.",
    "The L1 batch relayer persists pending/submitted/confirmed/failed status, uses bounded retries, submits signed external BoCs through Toncenter v3 `/message`, and observes confirmation through `/transactionsByMessage`.",
    "The L1 batch finalizer persists a separate pending/submitted/finalized/failed queue; it creates finalization work only after local commit confirmation and waits local confirmation time + challengeWindowSec before signing.",
    "Relayer/operator visibility should expose failed batch records with safe static reason codes and no raw provider payloads or signer secrets.",
    "The node should not hold raw TON wallet credentials for relaying/finalization; use a separate typed signer service and verify returned signer address, expiry, and BoC shape before broadcasting.",
    "Signer service actions are allowlisted typed actions: MVP supports commit_batch on `/sign-commit` and finalize_batch on `/sign-finalize`; configure L2_SIGNER_ROLLUP_ROOT_ADDRESS so a token cannot sign for an unexpected RollupRoot.",
    "Withdrawals: L2 creates withdrawal leaves; after batch finalization, user submits a ReleaseAuthorized leaf cell + compact Merkle proof to RollupRoot; root sends ReleaseAuthorized to AssetVault.",
    "The node proof API must gate withdrawal proofs on l1_batch_finalizations.status=finalized for the containing batch; pre-finalization requests return a safe 409 instead of usable claim material.",
    "The SDK builds Withdraw(assetId, amount, l1Recipient) L2 transactions and ClaimWithdrawal bodies from API proofs; it serializes WithdrawalMerkleProof as leafIndex:uint64, siblingsCount:uint16, and nullable Cell<WithdrawalProofChunk> refs with up to 3 siblings per chunk.",
    "Wallet-assisted withdrawal claims should send a TON Connect raw message to RollupRoot with the generated ClaimWithdrawal BoC as base64 payload and an operator-chosen msg value.",
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
    "SDK browser examples must not include admin bearer tokens; admin-only faucet helpers are for operator scripts or demo backends.",
    "Local browser/operator tooling may accept an admin token at runtime for operator panels but must not store it in localStorage, sessionStorage, generated bundles, or config files.",
    "Acton wallet metadata such as wallets.toml/global.wallets.toml and signer commands are local-only; prefer keyring or mnemonic-env and never commit mnemonic material.",
    "Postgres persists L2 blocks, transactions, deposits, withdrawals, L1 cursors, batch DA payload mirrors and public refs, L1 batch commit relay state, L1 batch finalization state, observer replay checkpoints, and ENT faucet grants; Redis owns public mempool replay, nonce locks, per-account queue counters, rate-limit counters, and sequencer leader-lock responsibilities."
  ],
  security_patterns: [
    "Use explicit admin/sequencer authorization and pausability for emergency response.",
    "Root-to-vault deployment linking must be admin-only, reject the zero sentinel, reject replay after first link, and happen before any batch commitment.",
    "RollupRoot must reject CommitBatch while the AssetVault address is still the zero sentinel, because linking is intentionally disabled after the first committed batch.",
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
    "Run cargo test for Rust changes; run scripts/ci/acton_contract_checks.sh for Tolk/Acton changes through Linux, WSL, CI, or Docker fallback.",
    "Keep source, tests, lockfiles, and docs in Git; ignore generated artifacts, caches, local databases, and secret material.",
    "When building new L2 features, document architecture, message flow, state model, bridge impact, Acton commands, risks, and limitations."
  ]
}
```

## Open Knowledge Gaps

- Exact fraud-proof VM design for replaying TON-compatible L2 transitions on L1 is not implemented.
- Production data availability backend is not finalized: TON Storage vs external DA vs hybrid.
- Wrapped-gas release remains future work; registered Jetton releases route through vault-owned Jetton wallets.
- Acton cannot be run from native PowerShell in this environment; use WSL or Docker for contract checks.
