import type { ChatBackendKind } from "./chat";

export type AskUserMode = "confirm" | "single" | "multi";

export interface AskUserOption {
  id?: string;
  label: string;
  description?: string;
  preview?: string;
  icon?: string;
  recommended?: boolean;
  danger?: boolean;
}

export interface AskUserQuestion {
  id: string;
  header?: string;
  question: string;
  mode: AskUserMode;
  options?: AskUserOption[];
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  skippable?: boolean;
  allowOther?: boolean;
  minSelections?: number;
  maxSelections?: number;
}

export interface AskUserSpec {
  title?: string;
  source?: string;
  intent?: "plan_approval";
  dismissable?: boolean;
  questions: AskUserQuestion[];
}

export interface AskUserAnswer {
  questionId: string;
  value: "yes" | "no" | "other" | string | string[];
  notes?: string;
  skipped?: boolean;
}

export interface AskUserResult {
  answers: Record<string, AskUserAnswer>;
  cancelled: boolean;
}

export interface AgentAskUserRequestEvent {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  spec: AskUserSpec;
}

export interface AgentAskUserResponsePayload {
  taskId: string;
  requestId: string;
  result: AskUserResult;
}
