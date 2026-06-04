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

export type PermissionMode = "full" | "ask" | "readonly";

export type ChatBackendKind = "claude" | "codex";

export interface ChatComposerState {
  taskId: string;
  backend: ChatBackendKind;
  model: string;
  planMode: boolean;
  permission: PermissionMode;
}

export interface ChatModelOption {
  id: string;
  label: string;
  backend: ChatBackendKind;
}

export type ToolConsentDecision = "allow" | "deny";
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
}

export interface ToolConsentResponsePayload {
  taskId: string;
  requestId: string;
  decision: ToolConsentDecision;
  message: string | null;
  updatedInput?: ToolConsentUpdatedInput;
}

export interface AgentInteractionSettings {
  nonInterruptMode: boolean;
  debug: boolean;
}
