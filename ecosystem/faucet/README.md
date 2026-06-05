# Entropis Faucet

Public Vue frontend and Fastify TypeScript backend for Entropis testnet ENT grants.

```bash
npm install
npm run build
npm run start
```

The server listens on `http://127.0.0.1:3002` by default and serves both the
Vue frontend and `/api/*` routes.

## GitHub OAuth

Create a GitHub OAuth app and configure:

- Homepage URL: `http://127.0.0.1:3002`
- Authorization callback URL: `http://127.0.0.1:3002/api/auth/github/callback`
- Client ID: store in `GITHUB_CLIENT_ID`
- Client secret: store in `GITHUB_CLIENT_SECRET`

Keep the client secret server-side only. The faucet exchanges the GitHub code
on the backend, validates `state`, uses PKCE, stores only a short-lived session
cookie, and never exposes GitHub access tokens to browser code.

For real grants, set `L2_ADMIN_TOKEN` and `ENTROPIS_API_URL`. The browser never
calls admin endpoints directly.
