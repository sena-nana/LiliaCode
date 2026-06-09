import type { CodexComposerSettings, CodexProfileSettings } from "./provider";

export type ChatRole = "user" | "assistant" | "system";

export type ChatAttachmentKind = "file" | "directory" | "unknown";

export interface ChatAttachmentDirectoryMeta {
  fileCount: number;
  directoryCount: number;
  totalSize: number;
  truncated: boolean;
  unreadableCount: number;
}

export interface ChatAttachment {
  id: string;
  name: string;
  path: string;
  kind: ChatAttachmentKind;
  size: number | null;
  exists?: boolean;
  mime?: string | null;
  directory?: ChatAttachmentDirectoryMeta | null;
}

export interface ChatContextSearchResult {
  attachment: ChatAttachment;
  relativePath: string;
  matchedBy: "name" | "path";
}

export interface ChatMessage {
  id: string;
  taskId: string;
  role: ChatRole;
  content: string;
  attachments: ChatAttachment[];
  createdAt: number;
}

export type ChatSendDispatch = "started" | "queued";

export interface ChatSendResult {
  message: ChatMessage;
  dispatch: ChatSendDispatch;
  queuedCount: number;
}

export type CodexReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch"; branch: string }
  | { type: "commit"; sha: string };

export interface CodexReviewWorkflow {
  type: "codex_review";
  target: CodexReviewTarget;
  instructions?: string;
  delivery?: "inline" | "detached";
}

export type CodexGoalStatus =
  | "active"
  | "paused"
  | "blocked"
  | "usageLimited"
  | "budgetLimited"
  | "complete";

export interface CodexGoalWorkflow {
  type: "codex_goal";
  action: "set" | "refresh" | "clear";
  objective?: string;
  status?: CodexGoalStatus;
  tokenBudget?: number | null;
}

export interface CodexCompactWorkflow {
  type: "codex_compact";
}

export interface CodexBackgroundTerminalsCleanWorkflow {
  type: "codex_background_terminals_clean";
}

export type CodexMemoryMode = "enabled" | "disabled";

export interface CodexMemoryModeWorkflow {
  type: "codex_memory_mode";
  mode: CodexMemoryMode;
}

export interface CodexMemoryResetWorkflow {
  type: "codex_memory_reset";
}

export interface CodexThreadForkWorkflow {
  type: "codex_thread_fork";
  excludeTurns?: boolean;
}

export interface CodexConfigDiagnosticsWorkflow {
  type: "codex_config_diagnostics";
  includeLayers?: boolean;
}

export interface CodexThreadGoal {
  threadId: string;
  objective: string;
  status: CodexGoalStatus;
  tokenBudget: number | null;
  tokensUsed: number;
  timeUsedSeconds: number;
  createdAt: number;
  updatedAt: number;
}

export type ChatWorkflow =
  | CodexReviewWorkflow
  | CodexGoalWorkflow
  | CodexCompactWorkflow
  | CodexBackgroundTerminalsCleanWorkflow
  | CodexMemoryModeWorkflow
  | CodexMemoryResetWorkflow
  | CodexThreadForkWorkflow
  | CodexConfigDiagnosticsWorkflow;

export interface ChatInterruptResult {
  rolledBack: boolean;
  restoredContent: string;
  restoredAttachments: ChatAttachment[];
  removedEventIds: string[];
}

export type PermissionMode = "full" | "ask" | "readonly";

export type ChatBackendKind = "claude" | "codex";

export interface ChatComposerState {
  taskId: string;
  backend: ChatBackendKind;
  model: string;
  planMode: boolean;
  permission: PermissionMode;
  codexSettings?: CodexComposerSettings;
}

export interface ChatModelOption {
  id: string;
  label: string;
  backend: ChatBackendKind;
}

export type ToolConsentDecision = "allow" | "deny";
export type CodexToolConsentDecision = "accept" | "decline" | "cancel" | (string & {});
export type ToolConsentUpdatedInput = Record<string, unknown>;

export interface ToolConsentRequest {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  toolName: string;
  input: ToolConsentUpdatedInput;
  title: string | null;
  displayName: string | null;
  description: string | null;
  blockedPath: string | null;
  decisionReason: string | null;
  toolUseId: string | null;
  additionalPermissions?: unknown;
  availableDecisions?: CodexToolConsentDecision[];
  proposedExecpolicyAmendment?: unknown;
  proposedNetworkPolicyAmendments?: unknown;
  networkApprovalContext?: unknown;
  cwd?: string | null;
  reason?: string | null;
  commandActions?: unknown;
}

export interface ToolConsentResponsePayload {
  taskId: string;
  requestId: string;
  decision: ToolConsentDecision;
  message: string | null;
  updatedInput?: ToolConsentUpdatedInput;
  codexDecision?: CodexToolConsentDecision;
}

export interface AgentInteractionSettings {
  nonInterruptMode: boolean;
  debug: boolean;
  codexProfile: CodexProfileSettings;
}
