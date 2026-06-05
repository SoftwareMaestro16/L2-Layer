import { createHash, randomBytes } from "node:crypto"

import type { GitHubUser } from "./types.js"

type OAuthConfig = {
  clientId: string
  clientSecret: string
}

type AuthorizationParams = {
  redirectUri: string
  state: string
  codeChallenge: string
}

export class GitHubOAuthClient {
  constructor(
    private readonly config: OAuthConfig,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {}

  authorizationUrl(params: AuthorizationParams) {
    const url = new URL("https://github.com/login/oauth/authorize")
    url.searchParams.set("client_id", this.config.clientId)
    url.searchParams.set("redirect_uri", params.redirectUri)
    url.searchParams.set("state", params.state)
    url.searchParams.set("scope", "read:user")
    url.searchParams.set("code_challenge", params.codeChallenge)
    url.searchParams.set("code_challenge_method", "S256")
    return url.toString()
  }

  async completeCallback(params: { code: string; redirectUri: string; codeVerifier: string }) {
    const token = await this.exchangeCode(params.code, params.redirectUri, params.codeVerifier)
    return this.fetchUser(token)
  }

  private async exchangeCode(code: string, redirectUri: string, codeVerifier: string) {
    const response = await this.fetchImpl("https://github.com/login/oauth/access_token", {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        client_id: this.config.clientId,
        client_secret: this.config.clientSecret,
        code,
        redirect_uri: redirectUri,
        code_verifier: codeVerifier,
      }),
    })
    const body = (await response.json()) as { access_token?: unknown }

    if (!response.ok || typeof body.access_token !== "string") {
      throw new Error("github_oauth_failed")
    }

    return body.access_token
  }

  private async fetchUser(token: string): Promise<GitHubUser> {
    const response = await this.fetchImpl("https://api.github.com/user", {
      headers: {
        accept: "application/vnd.github+json",
        authorization: `Bearer ${token}`,
        "user-agent": "entropis-faucet",
      },
    })
    const body = (await response.json()) as {
      id?: unknown
      login?: unknown
      avatar_url?: unknown
    }

    if (!response.ok || typeof body.id !== "number" || typeof body.login !== "string") {
      throw new Error("github_user_failed")
    }

    return {
      id: body.id,
      login: body.login,
      avatarUrl: typeof body.avatar_url === "string" ? body.avatar_url : null,
    }
  }
}

export function createPkcePair() {
  const codeVerifier = base64Url(randomBytes(32))
  const codeChallenge = base64Url(createHash("sha256").update(codeVerifier).digest())
  return { codeVerifier, codeChallenge }
}

function base64Url(bytes: Buffer) {
  return bytes.toString("base64url")
}
