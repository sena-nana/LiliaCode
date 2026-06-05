export type SuggestionSource = "provider" | "assistant-ai";

export interface SuggestionItem {
  id: string;
  projectId: string | null;
  taskIds: string[];
  summary: string;
  reason: string;
  prompt: string;
  generatedAt: number;
}

export interface SuggestionSettings {
  enabled: boolean;
  source: SuggestionSource;
}
