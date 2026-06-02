/**
 * Timeline display 派生器：纯函数，从 `{kind, status, title, summary, payload}`
 * 算出 `AgentTimelineDisplay`。前端在渲染时调用，runner / Rust 端只负责存事实
 * （kind + subkind + lilia payload），不固化任何展示文本，这样改 display 规则
 * 可以即时影响历史数据。
 *
 * 工具事件按 lilia 协议（liliaTools.mjs）派生：先取 payload.kind 兜底（旧 DB 数据
 * payload.toolName=Bash 这类的，再走 Claude 适配层 normalize 一次回填）。其它 kind
 * （message/reasoning/mcp/error/turn/...）走本文件 buildByKind 分支。
 */
import {
  deriveLiliaToolDisplay,
  getLiliaToolRule,
  compactLine,
  pick,
  readFirstString,
  readRecord,
  displayField,
  fieldsDetail,
  codeDetail,
  markdownDetail,
  isFailureStatus,
} from "./liliaTools.mjs";
import { normalizeClaudeTool } from "./claudeTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineDisplayListItem,
  AgentTimelineEventStatus,
  AgentTimelinePayload,
} from "./index";

export interface TimelineDisplayInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string | null;
  payload: AgentTimelinePayload;
  projectCwd?: string | null;
}

export function deriveTimelineDisplay(input: TimelineDisplayInput): AgentTimelineDisplay {
  const kind = input.kind || "tool";
  const title = compactLine(input.title, 200) || kind;
  const summary = compactLine(input.summary ?? "", 1200);
  const payload = readRecord(input.payload);
  const context = createDisplayContext(input.projectCwd);

  const toolDisplay = tryDeriveToolDisplay(kind, payload, title, summary, input.status);
  const display = toolDisplay
    ? cleanDisplay(toolDisplay)
    : cleanDisplay(buildByKind({ kind, status: input.status, title, summary, payload }));
  return applyDisplayContext(
    display ?? fallbackDisplay(kind, title, summary),
    context,
  );
}

const AGENT_TIMELINE_TOOL_WINDOW_KINDS = new Set([
  "tool",
  "command",
  "mcp",
  "search",
  "web_search",
  "web_fetch",
  "file_read",
  "file_change",
  "todo_list",
  "subagent",
]);

export function isAgentTimelineToolWindowKind(kind: string): boolean {
  return AGENT_TIMELINE_TOOL_WINDOW_KINDS.has(kind);
}

/**
 * 试着按 lilia 工具协议派生 display。命中两类输入：
 *  - 旧 DB 事件：`payload.toolName` 是 Claude 工具名（如 Bash/Read/Edit），lilia
 *    字段（command / path / query …）藏在 `payload.input` 里。通过
 *    `normalizeClaudeTool` 把 input 折成 lilia 协议字段，再补回 payload。
 *  - 新协议事件：`kind` 已经是 lilia 工具 kind，payload 顶层就有 command/path/query
 *    等字段。Claude normalizer 看不见 `payload.input`（没那字段），会返回空字段，
 *    必须用原始 payload 的值覆盖回来，否则派生器只剩下 output 可用，preview 就
 *    退化成结果文本。
 */
function tryDeriveToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
  status: AgentTimelineEventStatus,
): AgentTimelineDisplay | null {
  if (getLiliaToolRule(kind, asString(payload.subkind))) {
    const display = deriveLiliaToolDisplay({
      kind,
      subkind: asString(payload.subkind),
      payload,
      title,
      status,
    });
    return finishToolDisplay(display, payload, title, summary);
  }

  return tryDeriveLegacyClaudeToolDisplay(kind, payload, title, summary, status);
}

function tryDeriveLegacyClaudeToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
  status: AgentTimelineEventStatus,
): AgentTimelineDisplay | null {
  if (!isLegacyToolKind(kind)) return null;
  const toolName = readFirstString(payload, ["toolName", "tool", "name"], 200);
  if (!toolName) return null;

  const normalized = normalizeClaudeTool(toolName, payload.input, payload);
  if (normalized.kind === "tool" && kind !== "tool") return null;

  const display = deriveLiliaToolDisplay({
    kind: normalized.kind,
    subkind: normalized.subkind,
    payload: legacyToolPayload(normalized.payload, payload),
    title,
    status,
  });
  return finishToolDisplay(display, payload, title, summary);
}

function legacyToolPayload(
  normalized: Record<string, unknown>,
  original: Record<string, unknown>,
): Record<string, unknown> {
  const allowedOriginalFields = [
    "approved",
    "cancelled",
    "decision",
    "duration",
    "error",
    "exit",
    "exitCode",
    "message",
    "output",
    "result",
    "stderr",
    "stdout",
    "structuredContent",
  ];
  const merged: Record<string, unknown> = { ...normalized };
  for (const key of allowedOriginalFields) {
    if (!(key in original)) continue;
    const value = original[key];
    if (key === "approved" && value === null) {
      merged[key] = value;
      continue;
    }
    if (value === undefined || value === null || value === "") continue;
    merged[key] = value;
  }
  return merged;
}

function finishToolDisplay(
  display: AgentTimelineDisplay | null,
  _payload: Record<string, unknown>,
  title: string,
  summary: string,
): AgentTimelineDisplay | null {
  if (!display) return null;
  const preview = summary || display.preview || title;
  const object = display.object?.trim() ? display.object : title;
  return {
    ...display,
    object,
    preview,
  };
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value ? value : null;
}

/** 旧持久化数据里把工具事件落进这些 kind，需要走兼容映射。 */
function isLegacyToolKind(kind: string): boolean {
  return isAgentTimelineToolWindowKind(kind) || kind === "plan";
}

interface KindBuildInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string;
  payload: Record<string, unknown>;
}

function buildByKind({ kind, status, title, summary, payload }: KindBuildInput): AgentTimelineDisplay {
  switch (kind) {
    case "message": {
      const role = readFirstString(payload, ["role"], 80);
      return {
        icon: "message-square",
        label: role === "assistant" ? "Assistant" : title,
        preview: summary || readFirstString(payload, ["content"], 600),
        defaultExpanded: role === "assistant" ? true : undefined,
      };
    }
    case "reasoning":
      return {
        action: "思考",
        preview: summary || readFirstString(payload, ["text", "summary"], 600),
        details: [markdownDetail(summary || pick(payload, ["text", "summary"]), "muted")]
          .filter((d): d is AgentTimelineDisplayDetail => d !== null),
      };
    case "mcp": {
      if (isCodexMcpConfigEvent(payload)) {
        const count = typeof payload.serverCount === "number" && Number.isFinite(payload.serverCount)
          ? payload.serverCount
          : Array.isArray(payload.servers)
            ? payload.servers.length
            : null;
        return {
          icon: "plug",
          label: "Codex MCP 配置",
          preview: summary || (count && count > 0
            ? `已发现 ${count} 个 MCP server`
            : "未发现 Codex MCP server"),
          details: [fieldsDetail([
            displayField("config", pick(payload, ["configPath"])),
          ])].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        };
      }
      const target = [
        readFirstString(payload, ["server", "serverName", "mcpServer"], 200),
        readFirstString(payload, ["tool", "toolName", "name"], 200),
      ]
        .filter(Boolean)
        .join("/");
      return {
        icon: "plug",
        action: "调用 MCP",
        object: target || usefulObject(title, ["mcp", "mcp tool"]),
        objectInLabel: true,
        preview: summary || target,
        details: [
          codeDetail("INPUT", pickValue(payload, [
            "input",
            "arguments",
            "args",
            "parameters",
            "params",
            "request",
          ])),
          codeDetail(
            isFailureStatus(status) ? "ERROR / OUTPUT" : "OUTPUT",
            pickValue(payload, ["result", "response", "output", "text", "content", "error", "message"]),
          ),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: {
          key: `mcp:${readFirstString(payload, ["server", "serverName", "mcpServer"], 120) || "default"}`,
          bucket: "mcp",
          unit: "次 MCP",
          count: 1,
        },
      };
    }
    case "error": {
      const message =
        summary ||
        readFirstString(payload, ["message", "error", "reason", "details", "stderr"], 1200);
      return {
        icon: "alert-triangle",
        label: title || "错误",
        preview: message,
        details: [
          lineDetail(message, "muted"),
          fieldsDetail([
            displayField("code", pick(payload, ["code", "exitCode", "statusCode"])),
            displayField("path", pick(payload, ["file", "filePath", "path"])),
            displayField("command", pick(payload, ["command", "cmd", "shellCommand"])),
          ]),
          codeDetail("STACK", pick(payload, ["stack", "trace", "backtrace"])),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:error", bucket: "error", unit: "个错误", count: 1 },
      };
    }
    case "turn":
      return {
        label: title,
        preview:
          summary || readFirstString(payload, ["status", "eventType", "subtype", "state"], 600),
        details: [
          lineDetail(summary),
          fieldsDetail([
            displayField("backend", pick(payload, ["backend"])),
            displayField(
              "event",
              pick(payload, ["eventType", "subtype", "status", "state"]),
            ),
            displayField("session", pick(payload, ["sessionId"])),
          ]),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
      };
    case "tool":
    default: {
      const tool =
        readFirstString(payload, ["toolName", "name", "tool", "function", "hookName"], 200) ||
        usefulObject(title, ["tool"]);
      const input = pickValue(payload, [
        "input",
        "arguments",
        "args",
        "parameters",
        "params",
        "request",
      ]);
      const output = pickValue(payload, ["result", "response", "output", "text", "content"]);
      return {
        icon: "wrench",
        action: kind === "tool" ? "调用工具" : "处理",
        object: tool || title,
        objectInLabel: true,
        preview:
          summary || tool || readFirstString(payload, ["query", "path", "command"], 600),
        details: [
          codeDetail("INPUT", input),
          codeDetail(isFailureStatus(status) ? "ERROR / OUTPUT" : "OUTPUT", output),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: `tool:${tool || title || kind}`, bucket: "tool", unit: "个工具", count: 1 },
      };
    }
  }
}

function isCodexMcpConfigEvent(payload: Record<string, unknown>): boolean {
  return payload.subkind === "config" || payload.source === "config.toml";
}

interface DisplayContext {
  projectCwd: string;
  projectCwdForCompare: string;
}

function createDisplayContext(projectCwd: string | null | undefined): DisplayContext | null {
  const normalized = normalizePath(projectCwd);
  if (!normalized || !isAbsolutePath(normalized) || isUrl(normalized)) return null;
  const caseInsensitive = isWindowsPath(normalized);
  return {
    projectCwd: normalized,
    projectCwdForCompare: normalizeForCompare(normalized, caseInsensitive),
  };
}

function applyDisplayContext(
  display: AgentTimelineDisplay,
  context: DisplayContext | null,
): AgentTimelineDisplay {
  if (!context) return display;
  return {
    ...display,
    object: shortenProjectPath(display.object, context) ?? display.object,
    preview: shortenProjectPath(display.preview, context) ?? display.preview,
    details: display.details?.map((detail) => applyDisplayContextToDetail(detail, context)),
  };
}

function applyDisplayContextToDetail(
  detail: AgentTimelineDisplayDetail,
  context: DisplayContext,
): AgentTimelineDisplayDetail {
  switch (detail.type) {
    case "fields":
      return {
        ...detail,
        fields: detail.fields.map((field) => ({
          ...field,
          value: shouldShortenField(field.label)
            ? shortenProjectPath(field.value, context) ?? field.value
            : field.value,
        })),
      };
    case "list":
      return {
        ...detail,
        items: detail.items.map((item) => ({
          ...item,
          text: shortenPathsInText(item.text, context),
        })),
      };
    default:
      return detail;
  }
}

function shouldShortenField(label: string): boolean {
  const normalized = label.trim().toLowerCase();
  return normalized === "path" || normalized === "cwd";
}

function shortenPathsInText(text: string, context: DisplayContext): string {
  const shortened = shortenProjectPath(text, context) ?? text;
  if (shortened !== text) return shortened;
  const prefix = `${context.projectCwd}/`;
  const index = normalizeForCompare(text.replace(/\\/g, "/"), isWindowsPath(context.projectCwd))
    .indexOf(context.projectCwdForCompare + "/");
  if (index < 0) return text;
  return `${text.slice(0, index)}${text.slice(index + prefix.length)}`.replace(/\\/g, "/");
}

function shortenProjectPath(
  value: string | null | undefined,
  context: DisplayContext | null,
): string | null | undefined {
  if (!value || !context) return value;
  const normalized = normalizePath(value);
  if (!normalized || isUrl(normalized) || !isAbsolutePath(normalized)) return value;
  const normalizedForCompare = normalizeForCompare(
    normalized,
    isWindowsPath(normalized) && isWindowsPath(context.projectCwd),
  );
  if (normalizedForCompare === context.projectCwdForCompare) return ".";
  const prefix = `${context.projectCwdForCompare}/`;
  if (!normalizedForCompare.startsWith(prefix)) return value;
  const relative = normalized.slice(context.projectCwd.length);
  return trimLeadingSeparators(relative) || ".";
}

function normalizePath(value: string | null | undefined): string {
  return (value ?? "").trim().replace(/\\/g, "/").replace(/\/+$/g, "");
}

function normalizeForCompare(value: string, caseInsensitive: boolean): string {
  return caseInsensitive ? value.toLowerCase() : value;
}

function isAbsolutePath(value: string): boolean {
  return /^[a-zA-Z]:\//.test(value) || value.startsWith("/");
}

function isWindowsPath(value: string): boolean {
  return /^[a-zA-Z]:\//.test(value);
}

function isUrl(value: string): boolean {
  return /^[a-zA-Z][a-zA-Z\d+.-]*:\/\//.test(value);
}

function trimLeadingSeparators(value: string): string {
  return value.replace(/^\/+/, "");
}

function fallbackDisplay(
  kind: string,
  title: string,
  summary: string,
): AgentTimelineDisplay {
  return {
    action: "处理",
    object: title || kind || "事件",
    objectInLabel: true,
    preview: summary || title || kind || "",
    group: {
      key: `kind:${kind || "event"}`,
      bucket: "other",
      unit: "项",
      count: 1,
    },
  };
}

function pickValue(record: Record<string, unknown>, keys: string[]): unknown {
  for (const key of keys) {
    if (record[key] !== undefined) return record[key];
  }
  return undefined;
}

function lineDetail(
  text: unknown,
  tone: "default" | "muted" = "muted",
): AgentTimelineDisplayDetail | null {
  const content = compactLine(text, 1200);
  return content ? { type: "line", text: content, tone } : null;
}

function usefulObject(title: string, generic: string[]): string {
  const text = compactLine(title, 300);
  if (!text) return "";
  const normalized = text.toLowerCase();
  return generic.map((g) => g.toLowerCase()).includes(normalized) ? "" : text;
}

function cleanDisplay(display: AgentTimelineDisplay | null): AgentTimelineDisplay | null {
  if (!display) return null;
  const entries = Object.entries(display).filter(([, value]) => {
    if (value === undefined || value === null) return false;
    if (typeof value === "string" && value === "") return false;
    if (Array.isArray(value) && value.length === 0) return false;
    return true;
  });
  return entries.length ? (Object.fromEntries(entries) as AgentTimelineDisplay) : null;
}

// AgentTimelineDisplayListItem 仅在 lineDetail/listDetail 内部被 .mjs 闭包构造，
// 这里 re-export 保证 .d.mts 类型链路完整。
export type { AgentTimelineDisplayListItem };
