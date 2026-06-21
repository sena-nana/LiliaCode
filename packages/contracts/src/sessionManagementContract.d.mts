export type SessionManagementAction =
  | "list"
  | "info"
  | "messages"
  | "rename"
  | "tag"
  | "delete"
  | "archive";

export interface NormalizedSessionManagementRuntimeCommand {
  action: SessionManagementAction;
  sessionId: string;
  title: string;
  tag: string | null;
  archived: boolean;
  limit: number;
  cursor: string | null;
  searchTerm: string;
  includeSystemMessages: boolean;
}

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

export const SESSION_MANAGEMENT_CONTRACT: {
  runtimeCommandType: "session_management";
  actions: readonly SessionManagementAction[];
  defaultLimit: number;
  maxLimit: number;
  defaultArchived: boolean;
  requiresSessionId: readonly SessionManagementAction[];
  requiresTitle: readonly SessionManagementAction[];
  requiresTagField: readonly SessionManagementAction[];
};

export const SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE: "session_management";
export const SESSION_MANAGEMENT_ACTIONS: readonly SessionManagementAction[];
export const DEFAULT_SESSION_MANAGEMENT_LIMIT: number;
export const MAX_SESSION_MANAGEMENT_LIMIT: number;
export const DEFAULT_SESSION_MANAGEMENT_ARCHIVED: boolean;

export function isSessionManagementAction(value: unknown): value is SessionManagementAction;

export function normalizeSessionManagementRuntimeCommand(
  value: unknown,
): NormalizedSessionManagementRuntimeCommand | null;

export function createSessionManagementRuntimeCommand(
  action: SessionManagementAction,
  options?: Partial<Omit<SessionManagementRuntimeCommand, "type" | "action">>,
): SessionManagementRuntimeCommand;
