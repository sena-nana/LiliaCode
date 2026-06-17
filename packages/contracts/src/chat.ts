import type { CodexProfileSettings } from "./provider";

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

export type ChatSlashCommandSource = "native" | "project";

export interface ChatSlashCommandParameter {
  name: string;
  label: string;
  required: boolean;
  hint?: string | null;
}

export interface ChatSlashCommand {
  id: string;
  name: string;
  title: string;
  description: string;
  source: ChatSlashCommandSource;
  parameters: ChatSlashCommandParameter[];
}

export interface ChatSlashCommandSearchResult {
  command: ChatSlashCommand;
  matchedBy: "name" | "title" | "description";
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
  turnId: string;
}

export type LiliaReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch"; branch: string }
  | { type: "commit"; sha: string };

export interface LiliaReviewWorkflow {
  type: "lilia_review";
  target: LiliaReviewTarget;
  instructions?: string;
  delivery?: "inline" | "detached";
}

export interface LiliaFixSuggestionWorkflow {
  type: "lilia_fix_suggestion";
  target: LiliaReviewTarget;
  instructions?: string;
  mode?: "suggest" | "apply";
}

export interface LiliaBatchApplyWorkflow {
  type: "lilia_batch_apply";
  sourceTurnId: string;
  sourceKind: "review" | "fix_suggestion";
  sourceSummary: string;
  instructions?: string;
}

export type LiliaGoalStatus =
  | "active"
  | "paused"
  | "blocked"
  | "usageLimited"
  | "budgetLimited"
  | "complete";

export interface LiliaGoalWorkflow {
  type: "lilia_goal";
  action: "set" | "refresh" | "clear";
  objective?: string;
  status?: LiliaGoalStatus;
  tokenBudget?: number | null;
}

export interface LiliaCompactWorkflow {
  type: "lilia_compact";
}

export interface LiliaBackgroundTerminalsCleanWorkflow {
  type: "lilia_background_terminals_clean";
}

export type LiliaMemoryMode = "enabled" | "disabled";

export interface LiliaMemoryModeWorkflow {
  type: "lilia_memory_mode";
  mode: LiliaMemoryMode;
}

export interface LiliaMemoryResetWorkflow {
  type: "lilia_memory_reset";
}

export type SessionManagementAction =
  | "list"
  | "info"
  | "messages"
  | "rename"
  | "tag"
  | "delete"
  | "archive";

export interface SessionManagementRuntimeCommand {
  type: "session_management";
  action: SessionManagementAction;
  sessionId?: string;
  title?: string;
  tag?: string | null;
  archived?: boolean;
  limit?: number;
  cursor?: string;
  searchTerm?: string;
  includeSystemMessages?: boolean;
}

export interface LiliaConfigDiagnosticsWorkflow {
  type: "lilia_config_diagnostics";
  includeLayers?: boolean;
}

export interface SessionForkRuntimeCommand {
  type: "session_fork";
  excludeTurns?: boolean;
}

export interface LiliaRuntimeSettingsCommon {
  model?: string;
  permission?: PermissionMode;
  reasoningEffort?: string;
  runtimeWorkspaceRoots?: string[];
}

export interface ProviderRuntimeOptionsCodex {
  profile?: string;
  model?: string | null;
  reasoningEffort?: string;
  runtimeWorkspaceRoots?: string[];
  additionalContext?: string | null;
  persistExtendedHistory?: boolean;
  initialTurnsPage?: Record<string, unknown> | null;
  excludeTurns?: string[];
  environments?: unknown[];
  experimentalRawEvents?: boolean;
  responsesApiClientMetadata?: Record<string, unknown>;
}

export interface ProviderRuntimeOptionsClaude {
  allowedTools?: string[];
  disallowedTools?: string[];
  additionalDirectories?: string[];
  maxTurns?: number;
  maxBudgetUsd?: number;
  tools?: string[] | { type: "preset"; preset: string };
  permissionPromptToolName?: string;
  settings?: string | Record<string, unknown>;
  managedSettings?: Record<string, unknown>;
  settingSources?: string[];
  sandbox?: Record<string, unknown>;
  outputFormat?: Record<string, unknown>;
  includeHookEvents?: boolean;
  forwardSubagentText?: boolean;
  agentProgressSummaries?: boolean;
  continue?: boolean;
  resumeSessionAt?: string;
  sessionId?: string;
  abortAfterMs?: number;
  sessionStore?: Record<string, unknown>;
}

export interface ExperimentalProviderOptions {
  provider: ChatBackendKind;
  capability: string;
  payload: Record<string, unknown>;
  fallback: "diagnostic" | "unsupported" | "ignore";
}

export interface ProviderRuntimeOptions {
  common?: LiliaRuntimeSettingsCommon;
  provider?: {
    codex?: ProviderRuntimeOptionsCodex;
    claude?: ProviderRuntimeOptionsClaude;
  };
  experimentalProviderOptions?: ExperimentalProviderOptions[];
}

export interface RuntimeSettingsCommand {
  type: "runtime_settings";
  action: "diagnose" | "update";
  common?: never;
  runtimeOptions?: never;
}

export interface AutomationRunWorkflow {
  type: "automation";
  automationRunId: string;
}

export interface ChatSlashCommandWorkflow {
  type: "slash_command";
  commandId: string;
  source: ChatSlashCommandSource;
  arguments: Record<string, string>;
}

export interface LiliaThreadGoal {
  threadId: string;
  objective: string;
  status: LiliaGoalStatus;
  tokenBudget: number | null;
  tokensUsed: number;
  timeUsedSeconds: number;
  createdAt: number;
  updatedAt: number;
}

export type ChatWorkflow =
  | LiliaReviewWorkflow
  | LiliaFixSuggestionWorkflow
  | LiliaBatchApplyWorkflow
  | LiliaGoalWorkflow
  | LiliaCompactWorkflow
  | LiliaBackgroundTerminalsCleanWorkflow
  | LiliaMemoryModeWorkflow
  | LiliaMemoryResetWorkflow
  | LiliaConfigDiagnosticsWorkflow
  | AutomationRunWorkflow
  | ChatSlashCommandWorkflow;

export type ChatRuntimeCommand =
  | SessionForkRuntimeCommand
  | SessionManagementRuntimeCommand
  | RuntimeSettingsCommand;

export interface ChatInterruptResult {
  rolledBack: boolean;
  restoredContent: string;
  restoredAttachments: ChatAttachment[];
  removedEventIds: string[];
}

export interface ChatRollbackResult {
  rolledBack: boolean;
  restoredContent: string;
  restoredAttachments: ChatAttachment[];
  removedEventIds: string[];
}

export type ChatRuntimePhase =
  | "idle"
  | "running"
  | "queued"
  | "running_and_queued"
  | "interrupted_pending_finish"
  | "reset_pending_finish"
  | "abandoned";

export interface ChatContextUsage {
  taskId: string;
  backend: ChatBackendKind;
  usedTokens: number;
  limitTokens: number | null;
  usedPercent: number | null;
  source: string;
  updatedAt: number;
  unavailableReason?: string | null;
}

export interface ChatRuntimeSnapshot {
  taskId: string;
  phase: ChatRuntimePhase;
  backend: ChatBackendKind | null;
  turnId: string | null;
  queuedCount: number;
  pendingRollback: boolean;
  pendingResetCleanup: boolean;
  contextUsage: ChatContextUsage | null;
  rollback?: ChatRollbackResult | null;
}

export type PermissionMode = "full" | "ask" | "readonly";

export type ChatBackendKind = "claude" | "codex";

export interface ChatComposerState {
  taskId: string;
  backend: ChatBackendKind;
  model: string;
  planMode: boolean;
  goalMode: boolean;
  permission: PermissionMode;
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
