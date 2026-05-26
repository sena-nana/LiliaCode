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
  readTodoItems,
  displayField,
  fieldsDetail,
  codeDetail,
  markdownDetail,
  listDetail,
  type ParsedTodoItem,
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
}

export function deriveTimelineDisplay(input: TimelineDisplayInput): AgentTimelineDisplay {
  const kind = input.kind || "tool";
  const title = compactLine(input.title, 200) || kind;
  const summary = compactLine(input.summary ?? "", 1200);
  const payload = readRecord(input.payload);

  const toolDisplay = tryDeriveToolDisplay(kind, payload, title, summary);
  if (toolDisplay) {
    return cleanDisplay(toolDisplay) ?? fallbackDisplay(kind, title, summary);
  }

  return (
    cleanDisplay(buildByKind({ kind, status: input.status, title, summary, payload })) ??
    fallbackDisplay(kind, title, summary)
  );
}

/**
 * 试着按 lilia 工具协议派生 display。命中两类输入：
 *  - 旧 DB 事件：`payload.toolName` 是 Claude 工具名（如 Bash/Read/Edit）。先尝试
 *    通过 `normalizeClaudeTool` 把 (toolName, input) 折成 lilia 协议事件 ——
 *    历史数据 kind 通常是泛型 "tool"，但通过 toolName 能定位到正确的桶。
 *  - 新协议事件：`kind` 直接是 lilia 工具 kind（command / file_read / ... / tool），
 *    且没有 toolName（或 normalizer 没命中专用规则），按 kind+subkind 派生。
 */
function tryDeriveToolDisplay(
  kind: string,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
): AgentTimelineDisplay | null {
  const toolName = readFirstString(payload, ["toolName", "tool", "name"], 200);
  if (toolName && isLegacyToolKind(kind)) {
    const normalized = normalizeClaudeTool(toolName, payload.input, payload);
    // 仅当 normalizer 命中专用规则（kind 不是兜底的 "tool"）时才走这条路径，
    // 否则按下面的"事件自身 kind"分支处理（保留事件生产方声明的 kind）。
    if (normalized.kind !== "tool" || kind === "tool") {
      const display = deriveLiliaToolDisplay({
        kind: normalized.kind,
        subkind: normalized.subkind,
        payload: { ...normalized.payload, output: pick(payload, ["output"]) },
        title,
      });
      const finished = finishToolDisplay(display, payload, title, summary);
      if (finished) return finished;
    }
  }

  if (getLiliaToolRule(kind, asString(payload.subkind))) {
    const display = deriveLiliaToolDisplay({
      kind,
      subkind: asString(payload.subkind),
      payload,
      title,
    });
    return finishToolDisplay(display, payload, title, summary);
  }

  return null;
}

function finishToolDisplay(
  display: AgentTimelineDisplay | null,
  payload: Record<string, unknown>,
  title: string,
  summary: string,
): AgentTimelineDisplay | null {
  if (!display) return null;
  // summary 是事件生产方的人工摘要（如 "正在运行完整验证"），优先于派生器自动算的
  // object 当 preview；object/output/title 做兜底。
  const preview =
    summary || display.preview || compactLine(pick(payload, ["output"]), 600) || title;
  // 派生器只读 payload；当 payload 没给出可用 object 时回退到事件 title。
  // 主要服务于 plan / todo_list 这类 payload 没有"目标对象"的事件，让 aria-label
  // 能拼出 "已更新待办 + title"。
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
  return (
    kind === "tool" ||
    kind === "command" ||
    kind === "file_change" ||
    kind === "file_read" ||
    kind === "todo_list" ||
    kind === "subagent" ||
    kind === "plan" ||
    kind === "search" ||
    kind === "web_fetch" ||
    kind === "web_search"
  );
}

// ---------- kind 分支 ----------

interface KindBuildInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string;
  payload: Record<string, unknown>;
}

function buildByKind({ kind, title, summary, payload }: KindBuildInput): AgentTimelineDisplay {
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
    case "todo_list": {
      const items = readTodoItems(payload);
      return {
        icon: "list-checks",
        action: "更新待办",
        preview: summary || todoPreview(items),
        details: [lineDetail(summary), listDetail(items)]
          .filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:todo_list", bucket: "todo", unit: "次待办", count: 1 },
      };
    }
    case "command": {
      // 旧 DB 事件（runner 直接 emit kind=command，但还没 normalize 到 lilia 协议字段）
      // 兜底：从 payload 里取 command/output/stderr 这些字段拼显示，
      // 与 liliaTools.command.default 的渲染保持一致。
      const nestedInput = readRecord(payload.input);
      const command =
        readFirstString(payload, ["command", "cmd", "shellCommand", "script", "argv"], 1200) ||
        readFirstString(nestedInput, ["command", "cmd", "shellCommand", "script", "argv"], 1200);
      const output = readFirstString(
        payload,
        ["aggregatedOutput", "combinedOutput", "outputText", "stdout", "output"],
        6000,
      );
      const stderr = readFirstString(
        payload,
        ["stderr", "errorOutput", "message", "error"],
        6000,
      );
      return {
        icon: "terminal",
        action: "运行",
        object: command,
        preview: summary || command || output || stderr,
        details: [
          lineDetail(summary),
          fieldsDetail([
            displayField("cwd", pick(payload, ["cwd", "workdir", "workingDirectory"])),
            displayField("exit", pick(payload, ["exitCode", "code", "statusCode", "exit"])),
            displayField("duration", formatDuration(payload)),
          ]),
          codeDetail("COMMAND", command, "shell"),
          codeDetail(stderr ? "ERROR / OUTPUT" : "OUTPUT", output || stderr),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:command", bucket: "command", unit: "条命令", count: 1 },
      };
    }
    case "file_change": {
      const changes = readFileChanges(payload);
      const count = changes.length || 1;
      return {
        icon: "file-pen",
        action: "修改",
        object: fileChangeObject(changes, payload) || usefulObject(title, ["file change", "file changes"]),
        preview: summary || fileChangePreview(changes, payload),
        details: [
          lineDetail(summary),
          listDetail(changes.map((change) => `${change.kind} ${change.path}`)),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:file_change", bucket: "file", unit: "个文件", count },
      };
    }
    case "mcp": {
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
          fieldsDetail([
            displayField("服务", pick(payload, ["server", "serverName", "mcpServer"])),
            displayField("工具", pick(payload, ["tool", "toolName", "name"])),
          ]),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: {
          key: `mcp:${readFirstString(payload, ["server", "serverName", "mcpServer"], 120) || "default"}`,
          bucket: "mcp",
          unit: "次 MCP",
          count: 1,
        },
      };
    }
    case "subagent": {
      // 旧 DB 兜底（同 command/file_change）；新协议事件会先命中 lilia 工具规则。
      const name =
        readFirstString(
          payload,
          ["agentType", "subagentType", "agentName", "taskType", "name", "type"],
          200,
        ) || usefulObject(title, ["task", "agent"]);
      const task = readFirstString(
        payload,
        ["taskDescription", "description", "prompt", "task"],
        1200,
      );
      const result = readFirstString(payload, ["result", "output", "summary"], 1200);
      return {
        icon: "bot",
        action: "调用子代理",
        object: name,
        preview: summary || [name, task].filter(Boolean).join(": "),
        details: [
          markdownDetail(task, "default"),
          markdownDetail(result, "default"),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: "kind:subagent", bucket: "subagent", unit: "个子代理", count: 1 },
      };
    }
    case "plan": {
      const plan = readFirstString(payload, ["plan", "content", "text"], 6000);
      return {
        icon: "list-ordered",
        action: "制定计划",
        object: title,
        preview: summary || plan,
        details: [markdownDetail(plan || summary)].filter(
          (d): d is AgentTimelineDisplayDetail => d !== null,
        ),
        group: { key: "kind:plan", bucket: "plan", unit: "项计划", count: 1 },
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
          fieldsDetail([
            displayField("工具", tool),
            displayField("服务", pick(payload, ["server", "serverName", "mcpServer"])),
          ]),
          codeDetail("INPUT", input),
          codeDetail("OUTPUT", output),
        ].filter((d): d is AgentTimelineDisplayDetail => d !== null),
        group: { key: `tool:${tool || title || kind}`, bucket: "tool", unit: "个工具", count: 1 },
      };
    }
  }
}

function fallbackDisplay(kind: string, title: string, summary: string): AgentTimelineDisplay {
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

// ---------- TS-only helper（不需要跨到 runner） ----------

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
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

function formatDuration(payload: Record<string, unknown>): string {
  const raw = pickValue(payload, ["durationMs", "elapsedMs", "duration"]);
  if (typeof raw === "number") {
    return raw >= 1000 ? `${(raw / 1000).toFixed(1)}s` : `${raw}ms`;
  }
  return compactLine(raw, 80);
}

function todoPreview(items: ParsedTodoItem[]): string {
  if (!items.length) return "";
  const done = items.filter((item) => item.completed).length;
  const next = items.find((item) => !item.completed)?.text ?? "";
  return `${done}/${items.length} 已完成${next ? ` · ${next}` : ""}`;
}

interface FileChange {
  kind: string;
  path: string;
}

function readFileChanges(payload: Record<string, unknown>): FileChange[] {
  const input = readRecord(payload.input);
  const args = readRecord(payload.args);
  const parameters = readRecord(payload.parameters);
  const raw =
    (Array.isArray(payload.changes) && payload.changes) ||
    (Array.isArray(input.changes) && input.changes) ||
    (Array.isArray(args.changes) && args.changes) ||
    (Array.isArray(parameters.changes) && parameters.changes) ||
    [];
  return raw
    .map((change: unknown): FileChange | null => {
      if (!isRecord(change)) return null;
      const path = readFirstString(
        change,
        ["path", "filePath", "relativePath", "targetPath", "name"],
        600,
      );
      if (!path) return null;
      return {
        kind: readFirstString(change, ["kind", "operation", "type", "status"], 80) || "update",
        path,
      };
    })
    .filter((change): change is FileChange => change !== null);
}

function fileChangeObject(changes: FileChange[], payload: Record<string, unknown>): string {
  if (changes.length) return changes[0].path;
  return readFirstString(
    payload,
    ["path", "filePath", "relativePath", "targetPath", "name"],
    600,
  );
}

function fileChangePreview(changes: FileChange[], payload: Record<string, unknown>): string {
  if (changes.length) {
    const first = changes[0];
    const suffix = changes.length > 1 ? ` 等 ${changes.length} 个文件` : "";
    return `${first.kind} ${first.path}${suffix}`;
  }
  const path = fileChangeObject(changes, payload);
  if (!path) return "";
  const kind = readFirstString(payload, ["kind", "operation", "type", "status"], 80) || "update";
  return `${kind} ${path}`;
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
