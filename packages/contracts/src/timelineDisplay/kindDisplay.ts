import {
  codeDetail,
  displayField,
  fieldsDetail,
  isFailureStatus,
  markdownDetail,
  pick,
  readFirstString,
} from "../liliaTools.mjs";
import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineEventStatus,
} from "../timeline";
import { lineDetail, pickValue, usefulObject } from "./detailHelpers";

interface KindBuildInput {
  kind: string;
  status: AgentTimelineEventStatus;
  title: string;
  summary: string;
  payload: Record<string, unknown>;
}

export function buildByKind({ kind, status, title, summary, payload }: KindBuildInput): AgentTimelineDisplay {
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
        label: "发生错误",
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
