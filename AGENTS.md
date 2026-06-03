# Repository Agent Rules

## Default workflow

- Treat this repository as connected to `git@github.com:SoftwareMaestro16/L2-Layer.git`.
- After completing a meaningful change, run the relevant checks, commit the change, and push to `origin main` by default unless the user explicitly says not to push.
- Before every commit, inspect `git status --short` and `git diff --cached --name-only` to ensure generated artifacts, local state, secrets, wallets, mnemonics, API keys, and environment files are not staged.
- Keep `.gitignore` current when new tools introduce local caches, compiled outputs, database files, or credential material.

## TON L2 engineering rules

- Prefer official TON Docs for TON architecture, TVM, cells, messages, Jettons, and Tolk behavior.
- Prefer official Acton docs and the installed `acton --help` output for tool behavior. On Windows, Acton must run in WSL unless native support appears.
- Keep L1 contracts in Tolk and off-chain L2 components in Rust unless the user directs otherwise.
- Preserve deterministic execution: no randomness, wall-clock-dependent state transitions, unordered map iteration, or non-canonical serialization in the L2 state machine.
- Model bridge flows around TON's async message model. Jettons must follow the master + per-owner wallet contract model.
- Update `docs/TON_L2_SKILLS.md` whenever new TON L1, Tolk, Acton, Jetton, rollup, bridge, security, or sequencer knowledge changes implementation decisions.

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
