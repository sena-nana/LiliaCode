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

export interface LiliaSessionForkWorkflow {
  type: "lilia_session_fork";
  excludeTurns?: boolean;
}

export type LiliaSessionManagementAction =
  | "list"
  | "info"
  | "messages"
  | "rename"
  | "tag"
  | "delete"
  | "archive";

export interface LiliaSessionManagementWorkflow {
  type: "lilia_session_management";
  action: LiliaSessionManagementAction;
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

export interface LiliaProviderSettingsCommon {
  model?: string;
  permission?: PermissionMode;
}

export interface LiliaProviderSettingsCodex {
  profile?: string;
  reasoningEffort?: string;
  permissionProfile?: string;
  runtimeWorkspaceRoots?: string[];
  persistExtendedHistory?: boolean;
  environments?: unknown[];
  experimentalRawEvents?: boolean;
  responsesApiClientMetadata?: Record<string, unknown>;
}

export interface LiliaProviderSettingsClaude {
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

export interface LiliaProviderSettingsWorkflow {
  type: "lilia_provider_settings";
  action: "diagnose" | "update";
  common?: LiliaProviderSettingsCommon;
  codex?: LiliaProviderSettingsCodex;
  claude?: LiliaProviderSettingsClaude;
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
  | LiliaSessionForkWorkflow
  | LiliaSessionManagementWorkflow
  | LiliaConfigDiagnosticsWorkflow
  | LiliaProviderSettingsWorkflow
  | AutomationRunWorkflow
  | ChatSlashCommandWorkflow;

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

export interface ChatRuntimeSnapshot {
  taskId: string;
  phase: ChatRuntimePhase;
  runtimeChannel: AgentRuntimeChannel | null;
  backend: ChatBackendKind | null;
  turnId: string | null;
  queuedCount: number;
  pendingControlCount: number;
  pendingRollback: boolean;
  pendingResetCleanup: boolean;
  rollback?: ChatRollbackResult | null;
}

export type PermissionMode = "full" | "ask" | "readonly";

export type ChatBackendKind = "claude" | "codex";

export type AgentRuntimeChannel = "builtin" | "mutsuki_core";

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
  agentRuntimeChannel: AgentRuntimeChannel;
  codexProfile: CodexProfileSettings;
}
