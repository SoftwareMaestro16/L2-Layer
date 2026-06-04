# EnWatcher

Standalone public account and transaction explorer for Entropis L2.

## Run

```bash
npm ci
npm run dev
```

Open `http://localhost:3000`.

The explorer reads `NEXT_PUBLIC_ENTROPIS_API_BASE` when set and otherwise uses
`http://127.0.0.1:8080`. The API field in the top bar updates the base URL in
memory only.

## Checks

```bash
npm run typecheck
npm run lint
npm run test
```

## Public API

- `GET /healthz`
- `GET /v1/explorer/account/{id}`
- `GET /v1/explorer/account/{id}/transactions`
- `GET /v1/explorer/tx/{hash}`

No wallet connection, admin token, operator endpoint, secret, or local node state
is used by this app.
