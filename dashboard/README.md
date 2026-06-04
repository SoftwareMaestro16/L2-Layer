# Entropis Dashboard

Static explorer/operator dashboard for the Entropis testnet prototype.

Open `dashboard/index.html` in a browser and point the API field at a running
`l2-node`, usually `http://127.0.0.1:8080`.

Public sections call only public API endpoints:

- `GET /v1/explorer/summary`
- `GET /v1/explorer/blocks`
- `GET /v1/explorer/deposits`
- `GET /v1/tx/{hash}`
- `GET /v1/account/{id}`
- `GET /v1/explorer/deposit/{id}`
- `GET /v1/explorer/withdrawal/{id}`

The operator section requires the admin bearer token in the page input. The token
is kept only in memory and is not stored in local storage, session storage, or the
frontend bundle.

The registry field can point at a public deployment registry such as
`deployments/testnet/entropis.json` once testnet deployment metadata exists. When
RollupRoot and AssetVault addresses are present, the dashboard links them to
Tonviewer Testnet.
