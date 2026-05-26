// claudeTools.mjs 的类型声明 —— 让 TS 端 import 时拿到补全。
import type { LiliaToolKind, LiliaToolSubkind } from "./liliaTools.d.mts";

export interface NormalizedClaudeToolEvent {
  kind: LiliaToolKind;
  subkind: LiliaToolSubkind | null;
  payload: Record<string, unknown>;
  /** runner 写入 timeline event 的 summary 字段。 */
  summary: string;
}

export type ClaudeToolNormalizer = (
  input: Record<string, unknown>,
  ctx: Record<string, unknown> | null,
) => {
  kind: LiliaToolKind;
  subkind?: LiliaToolSubkind | null;
  payload: Record<string, unknown>;
  summary?: string;
};

/** Claude 工具名 → lilia 协议规范化器。 */
export const CLAUDE_TO_LILIA: Record<string, ClaudeToolNormalizer>;

/** 适配 Claude 工具调用为 lilia 协议事件；未登记工具走 tool 兜底 kind。 */
export function normalizeClaudeTool(
  name: string,
  input: unknown,
  ctx?: Record<string, unknown> | null,
): NormalizedClaudeToolEvent;
