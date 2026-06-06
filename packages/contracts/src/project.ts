import type { CodexComposerSettings } from "./provider";

export interface Project {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  pinned: boolean;
}

export type SessionKind = "interactive" | "headless" | "unknown";

export interface Session {
  sessionId: string;
  projectId: string;
  cwd: string;
  startedAt: number;
  kind: SessionKind;
  alive: boolean;
}

export type GitHubClientIdSource = "none" | "bundled" | "custom";

export interface GitHubBindingMetadata {
  login: string;
  avatarUrl: string | null;
  boundAt: number;
  scopes: string[];
  clientIdSource: Exclude<GitHubClientIdSource, "none">;
}

export interface GitHubBindingStatus {
  state: "unbound" | "bound";
  clientIdConfigured: boolean;
  clientIdSource: GitHubClientIdSource;
  binding: GitHubBindingMetadata | null;
}

export interface GitHubDeviceFlowStart {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresAt: number;
  intervalSeconds: number;
}

export interface GitHubDeviceFlowPollResult {
  status: "pending" | "authorized" | "expired";
  intervalSeconds: number;
  bindingStatus: GitHubBindingStatus | null;
  error: string | null;
}

export interface GitHubRepoSummary {
  id: number;
  name: string;
  fullName: string;
  ownerLogin: string;
  private: boolean;
  description: string | null;
  defaultBranch: string | null;
  updatedAt: string;
  cloneUrl: string;
  htmlUrl: string;
}

export interface GitHubRepoPage {
  items: GitHubRepoSummary[];
  nextPage: number | null;
}

export interface ProjectSettings {
  cloneParentDir: string | null;
  codexDefaults?: CodexComposerSettings | null;
  githubBinding?: GitHubBindingMetadata | null;
}

export interface PopupWindowSettings {
  shortcut: string | null;
}
