export type SuggestionSource = "provider" | "assistant-ai";
export type SuggestionItemSource = "task" | "github" | "local-git" | "claude";

export interface SuggestionGitHubActivityRef {
  id: string;
  repoFullName: string;
  kind: string;
  title: string;
  url: string | null;
}

export interface SuggestionLocalGitContextRef {
  id: string;
  branch: string;
  status: string;
  changedFiles: string[];
  recentCommits: string[];
}

export interface SuggestionItem {
  id: string;
  projectId: string | null;
  taskIds: string[];
  source: SuggestionItemSource;
  githubActivities: SuggestionGitHubActivityRef[];
  localGitContexts: SuggestionLocalGitContextRef[];
  summary: string;
  reason: string;
  prompt: string;
  generatedAt: number;
}

export interface SuggestionSettings {
  enabled: boolean;
  source: SuggestionSource;
}
