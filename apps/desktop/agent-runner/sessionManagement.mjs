import * as claudeSdk from "@anthropic-ai/claude-agent-sdk";
import {
  codexPreviewMessagesFromTurns,
  previewCodexThreadLiteWithServer,
  readCodexThreadTurns,
  renameCodexThreadWithServer,
  searchCodexThreadsWithServer,
} from "./codex/threadData.mjs";
import { isRecord, shortText, stringOrNull } from "./utils.mjs";

const SESSION_MANAGEMENT_ACTIONS = new Set(["list", "info", "messages", "rename"]);
const DEFAULT_LIMIT = 20;

function limitValue(value, fallback = DEFAULT_LIMIT, max = 100) {
  const number = Number(value);
  if (!Number.isFinite(number)) return fallback;
  return Math.max(1, Math.min(max, Math.trunc(number)));
}

function cursorOffset(value) {
  const number = Number(stringOrNull(value)?.trim() || 0);
  return Number.isFinite(number) && number > 0 ? Math.trunc(number) : 0;
}

function readSessionManagementWorkflow(cmd) {
  const workflow = isRecord(cmd?.workflow) ? cmd.workflow : null;
  if (workflow?.type !== "lilia_session_management") return null;
  const action = stringOrNull(workflow.action);
  if (!SESSION_MANAGEMENT_ACTIONS.has(action)) {
    throw new Error("Lilia session management workflow missing a valid action");
  }
  const sessionId = stringOrNull(workflow.sessionId)?.trim() || "";
  const title = stringOrNull(workflow.title)?.trim() || "";
  if ((action === "info" || action === "messages" || action === "rename") && !sessionId) {
    throw new Error("Lilia session management workflow missing sessionId");
  }
  if (action === "rename" && !title) {
    throw new Error("Lilia session management rename requires a non-empty title");
  }
  return {
    action,
    sessionId,
    title,
    limit: limitValue(workflow.limit),
    cursor: stringOrNull(workflow.cursor)?.trim() || null,
    searchTerm: stringOrNull(workflow.searchTerm)?.trim() || "",
    includeSystemMessages: workflow.includeSystemMessages === true,
  };
}

function emitSessionManagementTimeline(protocol, backend, workflow, status, config) {
  const failed = status === "error";
  protocol.emitTimeline({
    kind: "diagnostic",
    status,
    title: failed
      ? `${backend} session management failed`
      : `${backend} session management ${workflow.action}`,
    summary: failed
      ? config.error
      : config.summary || `${workflow.action} completed`,
    payload: {
      backend,
      source: "lilia",
      subkind: "session_management",
      action: workflow.action,
      native: config.native === true,
      sessionId: workflow.sessionId || null,
      threadId: backend === "codex" ? workflow.sessionId || config.threadId || null : null,
      result: config.result ?? null,
      ...(failed ? { error: config.error } : {}),
    },
    sourceId: `${backend}:session-management:${workflow.action}:${workflow.sessionId || "list"}:${Date.now()}`,
  });
}

function emitSessionManagementDone(protocol, workflow, fallbackSessionId = null) {
  protocol.emit({
    type: "done",
    sessionId: workflow.sessionId || fallbackSessionId || null,
    subtype: "success",
  });
}

function normalizeClaudeSessionInfo(info) {
  if (!isRecord(info)) return null;
  const sessionId = stringOrNull(info.sessionId) || stringOrNull(info.session_id);
  if (!sessionId) return null;
  return {
    id: sessionId,
    title: shortText(
      stringOrNull(info.summary) ||
        stringOrNull(info.title) ||
        stringOrNull(info.customTitle) ||
        "Claude session",
      160,
    ),
    sourceKind: "claude",
    createdAt: typeof info.createdAt === "number" ? info.createdAt : null,
    updatedAt: typeof info.lastModified === "number" ? info.lastModified : null,
    cwd: stringOrNull(info.cwd) || null,
    gitBranch: stringOrNull(info.gitBranch) || null,
    tag: stringOrNull(info.tag) || null,
  };
}

function contentText(content) {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .map((part) => {
      if (typeof part === "string") return part;
      if (!isRecord(part)) return "";
      if (typeof part.text === "string") return part.text;
      if (typeof part.content === "string") return part.content;
      return "";
    })
    .filter(Boolean)
    .join("\n");
}

function normalizeClaudeSessionMessage(message, index = 0) {
  if (!isRecord(message)) return null;
  const type = stringOrNull(message.type);
  const role = type === "assistant" ? "assistant" : type === "system" ? "system" : "user";
  const payload = isRecord(message.message) ? message.message : {};
  const text = contentText(payload.content ?? message.content ?? message.text);
  return {
    id: stringOrNull(message.uuid) || `${role}:${index}`,
    role,
    summary: shortText(text, 1200) || null,
  };
}

export async function runClaudeSessionManagementWorkflow(cmd, context, workingDir) {
  const workflow = readSessionManagementWorkflow(cmd);
  if (!workflow) return false;
  const options = { dir: workingDir };
  const safeContext = isRecord(context) ? context : {};
  const hasContextKey = (key) => Object.prototype.hasOwnProperty.call(safeContext, key);
  const sdk = {
    listSessions: hasContextKey("listClaudeSessions") ? safeContext.listClaudeSessions : claudeSdk.listSessions,
    getSessionInfo: hasContextKey("getClaudeSessionInfo") ? safeContext.getClaudeSessionInfo : claudeSdk.getSessionInfo,
    getSessionMessages: hasContextKey("getClaudeSessionMessages")
      ? safeContext.getClaudeSessionMessages
      : claudeSdk.getSessionMessages,
    renameSession: hasContextKey("renameClaudeSession") ? safeContext.renameClaudeSession : claudeSdk.renameSession,
  };
  try {
    let result;
    if (workflow.action === "list") {
      if (typeof sdk.listSessions !== "function") {
        throw new Error("Claude SDK listSessions is not available");
      }
      const sessions = await sdk.listSessions({
        ...options,
        limit: workflow.limit,
        offset: cursorOffset(workflow.cursor),
      });
      const normalized = (Array.isArray(sessions) ? sessions : [])
        .map(normalizeClaudeSessionInfo)
        .filter(Boolean);
      const nextOffset = cursorOffset(workflow.cursor) + normalized.length;
      result = {
        sessions: normalized.filter((session) => {
          const term = workflow.searchTerm.toLowerCase();
          if (!term) return true;
          return [session.id, session.title, session.cwd, session.gitBranch, session.tag]
            .some((value) => stringOrNull(value)?.toLowerCase().includes(term));
        }),
        nextCursor: normalized.length >= workflow.limit ? String(nextOffset) : null,
      };
    } else if (workflow.action === "info") {
      if (typeof sdk.getSessionInfo !== "function") {
        throw new Error("Claude SDK getSessionInfo is not available");
      }
      result = { session: normalizeClaudeSessionInfo(await sdk.getSessionInfo(workflow.sessionId, options)) };
    } else if (workflow.action === "messages") {
      if (typeof sdk.getSessionMessages !== "function") {
        throw new Error("Claude SDK getSessionMessages is not available");
      }
      const offset = cursorOffset(workflow.cursor);
      const messages = await sdk.getSessionMessages(workflow.sessionId, {
        ...options,
        limit: workflow.limit,
        offset,
        includeSystemMessages: workflow.includeSystemMessages,
      });
      const normalized = (Array.isArray(messages) ? messages : [])
        .map(normalizeClaudeSessionMessage)
        .filter(Boolean);
      result = {
        messages: normalized,
        nextCursor: normalized.length >= workflow.limit ? String(offset + normalized.length) : null,
      };
    } else {
      if (typeof sdk.renameSession !== "function") {
        throw new Error("Claude SDK renameSession is not available");
      }
      await sdk.renameSession(workflow.sessionId, workflow.title, options);
      result = { sessionId: workflow.sessionId, title: workflow.title, renamed: true };
    }
    emitSessionManagementTimeline(safeContext.protocol, "claude", workflow, "success", {
      native: true,
      result,
      summary: `Claude session management ${workflow.action} completed`,
    });
    emitSessionManagementDone(safeContext.protocol, workflow, stringOrNull(cmd.resumeSessionId));
  } catch (err) {
    emitSessionManagementTimeline(safeContext.protocol, "claude", workflow, "error", {
      native: false,
      error: err?.message || String(err),
    });
    throw err;
  }
  return true;
}

export async function runCodexSessionManagementWorkflow(
  server,
  threadId,
  cmd,
  ctx,
) {
  const workflow = readSessionManagementWorkflow(cmd);
  if (!workflow) return false;
  try {
    let result;
    if (workflow.action === "list") {
      result = await searchCodexThreadsWithServer(server, {
        limit: workflow.limit,
        archived: false,
        cursor: workflow.cursor,
        searchTerm: workflow.searchTerm,
      });
    } else if (workflow.action === "info") {
      result = await previewCodexThreadLiteWithServer(server, workflow.sessionId);
    } else if (workflow.action === "messages") {
      const { turns, nextCursor } = await readCodexThreadTurns(server, workflow.sessionId, {
        limit: workflow.limit,
        cursor: workflow.cursor,
      });
      result = {
        threadId: workflow.sessionId,
        messages: codexPreviewMessagesFromTurns(turns, workflow.limit),
        nextCursor,
      };
    } else {
      result = await renameCodexThreadWithServer(server, workflow.sessionId, workflow.title);
    }
    emitSessionManagementTimeline(ctx.protocol, "codex", workflow, "success", {
      native: true,
      result,
      threadId,
      summary: `Codex session management ${workflow.action} completed`,
    });
    emitSessionManagementDone(ctx.protocol, workflow, threadId);
  } catch (err) {
    emitSessionManagementTimeline(ctx.protocol, "codex", workflow, "error", {
      native: false,
      error: err?.message || String(err),
      threadId,
    });
    throw err;
  }
  return true;
}
