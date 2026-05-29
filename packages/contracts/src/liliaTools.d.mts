// liliaTools.mjs 的类型声明 —— 让 TS 端 import 时拿到补全。
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayField,
  AgentTimelineDisplayListItem,
  AgentTimelinePayload,
} from "./index";

/**
 * Lilia 工具协议主分类。所有 backend 的工具调用都必须被 normalize 成这些之一。
 */
export type LiliaToolKind =
  | "command"
  | "file_read"
  | "file_change"
  | "search"
  | "web_fetch"
  | "subagent"
  | "plan"
  | "todo_list"
  | "tool";

/** 子分类：仅在 file_change / search 下有意义。其余 kind subkind 应留空。 */
export type LiliaToolSubkind =
  // file_change
  | "edit"
  | "multi_edit"
  | "write"
  | "notebook"
  // search
  | "glob"
  | "grep"
  | "web";

/** 工具事件 payload 的协议字段并集 —— 实际 payload 是宽松的 Record。 */
export interface LiliaToolPayload {
  // command
  command?: string;
  cwd?: string;
  exit?: number | string;
  output?: string;
  stderr?: string;
  duration?: number | string;
  // file_read / file_change
  path?: string;
  offset?: number;
  limit?: number;
  editCount?: number;
  // search / web_fetch
  query?: string;
  glob?: string;
  url?: string;
  // subagent
  agentType?: string;
  description?: string;
  prompt?: string;
  result?: string;
  // plan
  plan?: string;
  allowedPrompts?: Array<{ tool?: string; prompt?: string }>;
  approved?: boolean | null;
  executionPermission?: string;
  revisionRequest?: string;
  // todo_list
  items?: Array<{ text: string; completed?: boolean; status?: string } | string>;
  // tool 兜底
  toolName?: string;
  input?: unknown;
}

export interface LiliaToolEvent {
  kind: LiliaToolKind;
  subkind?: LiliaToolSubkind | null;
  payload: LiliaToolPayload | Record<string, AgentTimelinePayload>;
}

export type LiliaToolDisplay = AgentTimelineDisplay;

export const LILIA_TOOL_KINDS: ReadonlyArray<LiliaToolKind>;

export interface LiliaToolRule {
  action: string;
  icon: string;
  bucket: string;
  unit: string;
  objectInLabel?: boolean;
  build: (
    payload: Record<string, unknown>,
  ) => {
    object: string;
    details: Array<AgentTimelineDisplayDetail | null>;
    label?: string;
    preview?: string;
    count?: number;
    defaultExpanded?: boolean;
  };
}

export function getLiliaToolRule(
  kind: string,
  subkind?: string | null,
): LiliaToolRule | null;

export function deriveLiliaToolDisplay(input: {
  kind: string;
  subkind?: string | null;
  payload: unknown;
  title?: string;
}): LiliaToolDisplay | null;

// ---------- helper（被派生器闭包内部使用，TS 端复用） ----------

export function readRecord(value: unknown): Record<string, unknown>;
export function pick(record: Record<string, unknown>, keys: string[]): unknown;
export function compactLine(value: unknown, max: number): string;
export function readFirstString(
  payload: Record<string, unknown>,
  keys: string[],
  max: number,
): string;
export function readFirstText(
  payload: Record<string, unknown>,
  keys: string[],
  max: number,
): string;
export function displayField(
  label: string,
  value: unknown,
): AgentTimelineDisplayField | null;
export function fieldsDetail(
  fields: Array<AgentTimelineDisplayField | null>,
): AgentTimelineDisplayDetail | null;
export function codeDetail(
  label: string,
  content: unknown,
  language?: string,
): AgentTimelineDisplayDetail | null;
export function markdownDetail(
  content: unknown,
  tone?: "default" | "muted",
  singleLine?: boolean,
): AgentTimelineDisplayDetail | null;
export function listDetail(
  items: unknown,
  ordered?: boolean,
): AgentTimelineDisplayDetail | null;

export interface ParsedTodoItem {
  text: string;
  completed: boolean;
  status?: string;
}

export function readTodoItems(payload: Record<string, unknown>): ParsedTodoItem[];
