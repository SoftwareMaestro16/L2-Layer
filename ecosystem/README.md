# Entropis Ecosystem

User-facing applications and integration packages around the Entropis L2 core.

- `explorer`: EnWatcher, a public read-only account and transaction explorer.
- `wallet`: reserved for wallet or Telegram Mini App flows.
- `PLAN.md`: local-only roadmap notes; public operator knowledge belongs under `docs/`.

Ecosystem apps consume public `l2-node` APIs. They must not read Postgres,
Redis, local wallets, signer tokens, mnemonics, or raw signed BoCs directly.
