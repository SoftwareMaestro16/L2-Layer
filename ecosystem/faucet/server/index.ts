import { createServer } from "node:http"

import { createFaucetApp } from "./app.js"
import { loadConfig } from "./config.js"
import { GitHubOAuthClient } from "./github.js"
import { EntropisNodeClient } from "./node-client.js"
import { FaucetStore } from "./store.js"
import { FaucetBatchWorker } from "./worker.js"

const config = loadConfig()
const store = new FaucetStore()
const githubClient =
  config.githubClientId && config.githubClientSecret
    ? new GitHubOAuthClient({
        clientId: config.githubClientId,
        clientSecret: config.githubClientSecret,
      })
    : null
const nodeClient = new EntropisNodeClient(config)
const worker = new FaucetBatchWorker(config, store, nodeClient)
const app = createFaucetApp({ config, store, githubClient })
const server = createServer(app)

worker.start()
server.listen(config.port, config.host, () => {
  console.log(`entropis faucet listening on http://${config.host}:${config.port}`)
})

function shutdown() {
  worker.stop()
  server.close(() => process.exit(0))
}

process.on("SIGINT", shutdown)
process.on("SIGTERM", shutdown)
