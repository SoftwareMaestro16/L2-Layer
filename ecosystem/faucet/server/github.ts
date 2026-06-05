import type { GitHubUser } from "./types.js";

export type GitHubOAuthConfig = {
  clientId: string;
  clientSecret: string;
};

export class GitHubOAuthClient {
  constructor(
    private readonly config: GitHubOAuthConfig,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {}

  authorizationUrl(params: { state: string; redirectUri: string }): string {
    const url = new URL("https://github.com/login/oauth/authorize");
    url.searchParams.set("client_id", this.config.clientId);
    url.searchParams.set("redirect_uri", params.redirectUri);
    url.searchParams.set("state", params.state);
    url.searchParams.set("scope", "read:user");
    return url.toString();
  }

  async completeCallback(params: { code: string; redirectUri: string }): Promise<GitHubUser> {
    const token = await this.exchangeCode(params.code, params.redirectUri);
    return this.fetchUser(token);
  }

  private async exchangeCode(code: string, redirectUri: string): Promise<string> {
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
      }),
    });
    const body = (await response.json()) as { access_token?: unknown; error?: unknown };
    if (!response.ok || typeof body.access_token !== "string") {
      throw new Error("github_oauth_failed");
    }
    return body.access_token;
  }

  private async fetchUser(token: string): Promise<GitHubUser> {
    const response = await this.fetchImpl("https://api.github.com/user", {
      headers: {
        accept: "application/vnd.github+json",
        authorization: `Bearer ${token}`,
        "user-agent": "entropis-faucet",
      },
    });
    const body = (await response.json()) as {
      id?: unknown;
      login?: unknown;
      avatar_url?: unknown;
    };
    if (!response.ok || typeof body.id !== "number" || typeof body.login !== "string") {
      throw new Error("github_user_failed");
    }
    return {
      id: body.id,
      login: body.login,
      avatarUrl: typeof body.avatar_url === "string" ? body.avatar_url : null,
    };
  }
}
