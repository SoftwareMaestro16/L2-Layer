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
const app = await createFaucetApp({ config, store, githubClient })

worker.start()
await app.listen({ host: config.host, port: config.port })
console.log(`entropis faucet listening on http://${config.host}:${config.port}`)

async function shutdown() {
  worker.stop()
  await app.close()
  process.exit(0)
}

process.on("SIGINT", () => {
  void shutdown()
})
process.on("SIGTERM", () => {
  void shutdown()
})
