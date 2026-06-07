export type SuggestionSource = "provider" | "assistant-ai";
export type SuggestionItemSource = "task" | "github";

export interface SuggestionGitHubActivityRef {
  id: string;
  repoFullName: string;
  kind: string;
  title: string;
  url: string | null;
}

export interface SuggestionItem {
  id: string;
  projectId: string | null;
  taskIds: string[];
  source: SuggestionItemSource;
  githubActivities: SuggestionGitHubActivityRef[];
  summary: string;
  reason: string;
  prompt: string;
  generatedAt: number;
}

export interface SuggestionSettings {
  enabled: boolean;
  source: SuggestionSource;
}
