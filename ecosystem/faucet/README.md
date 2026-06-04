# Entropis Faucet

Mock-only public faucet for Entropis testnet ENT tokens.

```bash
npm install
npm run dev
```

The Vite dev server runs on `http://127.0.0.1:3002`.

This package does not call `l2-node`, store secrets, or expose admin faucet
tokens. Real faucet integration should be added through a server-side service
that calls the admin-protected node primitive.
