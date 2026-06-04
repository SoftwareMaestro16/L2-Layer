# Entropis Ecosystem

User-facing applications and integration packages around the Entropis L2 core.

- `explorer`: public read-only account and transaction explorer.
- `wallet`: reserved for wallet or Telegram Mini App flows.
- `PLAN.md`: tracked roadmap for ecosystem boundaries and migrations.

Ecosystem apps consume public `l2-node` APIs. They must not read Postgres,
Redis, local wallets, signer tokens, mnemonics, or raw signed BoCs directly.
