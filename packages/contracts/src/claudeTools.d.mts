// claudeTools.mjs 的类型声明 —— 让 TS 端 import 时拿到补全。
import type {
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayField,
  AgentTimelineDisplayListItem,
} from "./index";

export interface ClaudeToolDisplay {
  action: string;
  icon: string;
  bucket: string;
  unit: string;
  objectInLabel?: boolean;
  extractObject: (input: Record<string, unknown>, name: string) => string;
  buildDetails: (
    input: Record<string, unknown>,
    payload: Record<string, unknown>,
    name: string,
  ) => Array<AgentTimelineDisplayDetail | null>;
}

export interface ClaudeToolDefinition {
  /** lilia 把工具调用归到哪类 timeline kind。 */
  kind: string;
  /** runner 从 input 取 timeline summary 的字段优先级。 */
  summaryFields: string[];
  /** timeline 渲染规则；deriveTimelineDisplay 调用 extractObject/buildDetails。 */
  display: ClaudeToolDisplay;
}

export const CLAUDE_TOOLS: Record<string, ClaudeToolDefinition>;

export const CLAUDE_TOOL_DEFAULT: ClaudeToolDefinition;

export function getClaudeTool(name: string): ClaudeToolDefinition;

// ---------- helper（buildDetails 闭包内部用，TS 端 deriver 也复用） ----------

export function readRecord(value: unknown): Record<string, unknown>;
export function pick(record: Record<string, unknown>, keys: string[]): unknown;
export function compactLine(value: unknown, max: number): string;
export function readFirstString(
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
