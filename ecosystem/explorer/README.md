# EnWatcher

Standalone public explorer and operator dashboard for Entropis L2.

## Run

```bash
npm ci
npm run dev
```

Open `http://localhost:3000`.

The explorer reads `NEXT_PUBLIC_ENTROPIS_API_BASE` when set and otherwise uses
`http://127.0.0.1:8080`. The API field in the top bar updates the base URL in
memory only.

Optional server-side operator and faucet settings:

```bash
ENTROPIS_API_URL=http://127.0.0.1:8080
L2_ADMIN_TOKEN=<node-admin-token>
ENWATCHER_OPERATOR_PASSWORD=<operator-dashboard-password>
FAUCET_API_URL=http://127.0.0.1:8090
```

`L2_ADMIN_TOKEN` is only read by Next route handlers under `/api/operator/*`.
It must not use a `NEXT_PUBLIC_` prefix and is never included in the browser
bundle.

## Checks

```bash
npm run typecheck
npm run lint
npm run test
npm run build
```

## Public API

- `GET /healthz`
- `GET /readyz`
- `GET /v1/explorer/summary`
- `GET /v1/explorer/blocks`
- `GET /v1/explorer/deposits`
- `GET /v1/explorer/deposit/{id}`
- `GET /v1/explorer/withdrawal/{id}`
- `GET /v1/explorer/account/{id}`
- `GET /v1/explorer/account/{id}/transactions`
- `GET /v1/explorer/tx/{hash}`
- `GET /v1/contract/{id}/state`
- `GET /v1/block/{height}`
- `GET /v1/block/{height}/finality`
- `GET /v1/da/batch/{height}`

## Security Notes

- Public pages use public node APIs only.
- Operator pages require `ENWATCHER_OPERATOR_PASSWORD`.
- Browser code never receives `L2_ADMIN_TOKEN`.
- EnWatcher does not read Postgres or Redis directly.
- Hashes, addresses, reasons, and JSON payloads are rendered as escaped React
  text, not injected HTML.
