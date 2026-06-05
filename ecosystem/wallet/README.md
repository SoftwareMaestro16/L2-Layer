# EnWallet

Entropis L2 testnet wallet UI.

## What works

- Creates a 24-word BIP39 seed phrase.
- Imports an existing 24-word seed phrase.
- Derives an Ed25519 keypair locally.
- Builds raw `8:<64 hex>` and user-friendly `EX...` Entropis L2 addresses.
- Stores the seed phrase in an encrypted IndexedDB vault derived from a local password.
- Locks/unlocks the in-memory wallet session without sending seed material to the server.
- Reads live balances and transaction history from the Entropis L2 node.
- Reviews, signs, and submits ENT transfers locally.
- Requests testnet ENT through a server-side faucet proxy when an admin token is configured.

## Environment

Create `ecosystem/wallet/.env.local` for local development:

```powershell
NEXT_PUBLIC_ENTROPIS_API_URL=http://127.0.0.1:8080
ENTROPIS_API_URL=http://127.0.0.1:8080
L2_ADMIN_TOKEN=<same value as root .env.local L2_ADMIN_TOKEN>
```

`NEXT_PUBLIC_ENTROPIS_API_URL` is visible to the browser. `L2_ADMIN_TOKEN` stays server-side and is only used by
`/api/faucet` and `/api/produce-block`.

## Commands

```powershell
npm ci
npm run typecheck
npm run lint
npm run build
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

This is testnet-only. Do not reuse a seed phrase or password from another wallet.
