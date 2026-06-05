# Entropis Faucet

GitHub-authenticated testnet faucet for L2-native ENT.

The browser app talks to a local Node backend. The backend stores sessions,
claims, cooldowns, pending queue, and batch history in RAM only. It calls
`l2-node` admin endpoints server-side; `L2_ADMIN_TOKEN` and GitHub OAuth tokens
are never sent to the browser.

## Run

```bash
npm ci
npm run dev:server
npm run dev
```

The Vite UI runs on `http://127.0.0.1:3002` and proxies `/api` to the backend on
`http://127.0.0.1:3003`.

## Backend Environment

```bash
ENTROPIS_API_URL=http://127.0.0.1:3000
L2_ADMIN_TOKEN=<node-admin-token>
GITHUB_CLIENT_ID=<github-oauth-client-id>
GITHUB_CLIENT_SECRET=<github-oauth-client-secret>
FAUCET_AMOUNT_ENT=100
FAUCET_BATCH_INTERVAL_MS=10000
FAUCET_COOLDOWN_SECONDS=7200
FAUCET_ENFORCE_COOLDOWN=false
FAUCET_MAX_BATCH_SIZE=100
```

`FAUCET_ENFORCE_COOLDOWN=false` is the current test default. Set it to `true`
to enforce one claim per GitHub account and one claim per L2 account every two
hours.

## Checks

```bash
npm run typecheck
npm test
npm run build
```
