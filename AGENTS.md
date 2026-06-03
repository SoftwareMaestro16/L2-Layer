# Repository Agent Rules

## Default workflow

- Treat this repository as connected to `git@github.com:SoftwareMaestro16/L2-Layer.git`.
- After completing a meaningful change, run the relevant checks, commit the change, and push to `origin main` by default unless the user explicitly says not to push.
- Use Conventional Commits for all new commits, for example `feat(l2-sequencer): add deterministic batch builder` or `test(bridge): add forged withdrawal proof case`.
- Start each feature with a branch suggestion even when the final push target is `origin/main`.
- Before every commit, inspect `git status --short` and `git diff --cached --name-only` to ensure generated artifacts, local state, secrets, wallets, mnemonics, API keys, and environment files are not staged.
- Keep `.gitignore` current when new tools introduce local caches, compiled outputs, database files, or credential material.

## TON L2 engineering rules

- Prefer official TON Docs for TON architecture, TVM, cells, messages, Jettons, and Tolk behavior.
- Prefer official Acton docs and the installed `acton --help` output for tool behavior. On Windows, Acton must run in WSL unless native support appears.
- Keep L1 contracts in Tolk and off-chain L2 components in Rust unless the user directs otherwise.
- Preserve deterministic execution: no randomness, wall-clock-dependent state transitions, unordered map iteration, or non-canonical serialization in the L2 state machine.
- Model bridge flows around TON's async message model. Jettons must follow the master + per-owner wallet contract model.
- Update `docs/TON_L2_SKILLS.md` whenever new TON L1, Tolk, Acton, Jetton, rollup, bridge, security, or sequencer knowledge changes implementation decisions.

## Code quality gates

- Keep human-authored source files below 500 lines. Split by responsibility before a file becomes a module catch-all.
- Generated wrappers may exceed the line limit, but must remain clearly generated and should not be hand-edited.
- Avoid hardcoded production values. Use config structs, manifest fields, environment placeholders, or genesis inputs.
- Keep business logic testable without network, wall-clock, or process-global state.
- Separate API, mempool, sequencer, executor, state, bridge, indexer, and config responsibilities.

## Security and test gates

- For every feature, consider replay, double spend, forged proof, state-root manipulation, malformed cell/message, gas griefing, mempool flooding, and sequencer censorship paths.
- Add or update unit tests for local behavior, adversarial tests for invalid inputs, integration tests for cross-module flows, and determinism tests for state roots.
- Run `cargo test` for Rust changes and `npm run typecheck` for SDK changes.
- Run `acton build`, `acton test`, `acton check`, and `acton fmt --check` for Tolk changes when Acton is available through WSL or Docker.

## Required response shape for build/design requests

When the user asks to build or design TON L2 functionality, cover:

1. Architecture: L1 and L2 split
2. Modules: Tolk contracts and Rust modules
3. Message flow: TON async model
4. State model: cells, BoC, Merkle commitments
5. Sequencer logic: Rust-level pseudocode when useful
6. L1 contract design: Tolk storage/messages/getters
7. Bridge design: deposits and withdrawals
8. Security assumptions
9. Acton CLI commands
10. Risks and limitations
