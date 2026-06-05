export type GitHubUser = {
  id: number;
  login: string;
  avatarUrl: string | null;
};

export type Session = {
  id: string;
  user: GitHubUser;
  expiresAt: number;
};

export type OAuthState = {
  state: string;
  redirectUri: string;
  expiresAt: number;
};

export type ClaimStatus = "pending" | "processing" | "granted" | "duplicate" | "failed";

export type FaucetClaim = {
  claimId: string;
  githubUserId: number;
  githubLogin: string;
  accountId: string;
  accountRawAddress: string;
  amountEnt: number;
  status: ClaimStatus;
  createdAt: number;
  updatedAt: number;
  attempts: number;
  lastError: string | null;
  nodeDepositId: string | null;
};

export type FaucetBatchStatus = "submitted" | "failed" | "partial";

export type FaucetBatch = {
  batchId: string;
  claimIds: string[];
  status: FaucetBatchStatus;
  createdAt: number;
  completedAt: number | null;
  error: string | null;
};

export type NodeClaimInput = {
  claimId: string;
  accountId: string;
};

export type NodeClaimResult = {
  claimId: string;
  status: "granted" | "duplicate" | "failed";
  depositId: string | null;
  error: string | null;
};
