# EnWatcher

Public read-only explorer for the Entropis L2 testnet prototype.

## Run

```bash
npm ci
npm run dev
```

The app reads `NEXT_PUBLIC_ENTROPIS_API_BASE` and defaults to `http://127.0.0.1:8080`.
The account QR send link reads `NEXT_PUBLIC_ENWALLET_URL` and defaults to `http://127.0.0.1:3001`.

## Checks

```bash
npm run typecheck
npm run lint
npm run test
npm run build
```
