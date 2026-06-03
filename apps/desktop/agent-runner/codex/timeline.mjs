import { buildPlanApprovalSpec, isPlanApprovalAccepted, readPlanRevisionRequest } from "../claudePlan.mjs";
import { createTextPacer } from "../protocol.mjs";
import {
  fullTextOrNull,
  isRecord,
  normalizeTimelineStatus,
  oneLineSummary,
  shortText,
  stringOrNull,
} from "../utils.mjs";

export function getCodexItemType(item) {
  return stringOrNull(item?.type) || "";
}

export function getCodexStatus(eventType, item) {
  const status = stringOrNull(item?.status);
  if (status) return normalizeTimelineStatus(status);
  if (eventType === "item.started") return "started";
  if (eventType === "item.completed") return "success";
  if (eventType === "turn.started") return "started";
  if (eventType === "turn.completed") return "success";
  if (eventType === "turn.failed" || eventType === "error") return "error";
  return "info";
}

export function codexTimelineKindForItem(item) {
  switch (getCodexItemType(item)) {
    case "reasoning":
      return { kind: "reasoning" };
    case "commandExecution":
      return { kind: "command" };
    case "fileChange":
      return { kind: "file_change" };
    case "mcpToolCall":
      return { kind: "mcp" };
    case "webSearch":
      return { kind: "search", subkind: "web" };
    case "plan":
      return { kind: "todo_list" };
    default:
      return null;
  }
}

export function summarizeCodexTodoList(items) {
  if (!Array.isArray(items)) return "";
  return items
    .map((todo) => {
      if (!isRecord(todo)) return shortText(todo, 120);
      const prefix = todo.completed ? "[x]" : "[ ]";
      return `${prefix} ${shortText(todo.text || todo.step, 160) || ""}`.trim();
    })
    .filter(Boolean)
    .join("\n");
}

export function summarizeCodexFileChanges(changes) {
  if (!Array.isArray(changes)) return "";
  return changes
    .map((change) => {
      if (!isRecord(change)) return shortText(change, 160);
      const path = shortText(change.path, 240) || "(unknown path)";
      return `${change.kind || "update"} ${path}`;
    })
    .filter(Boolean)
    .join("\n");
}

export function codexTimelineTitle(kind, item, eventType) {
  switch (kind) {
    case "reasoning":
      return "Reasoning";
    case "command":
      return shortText(item.command, 200) || "Command";
    case "file_change":
      return "File change";
    case "mcp":
      return [item.server, item.tool].filter(Boolean).join(" / ") || "MCP tool";
    case "search":
      return shortText(item.query, 200) || "Web search";
    case "todo_list":
      return eventType === "item.completed" ? "Plan completed" : "Plan";
    case "error":
      return "Error";
    default:
      return kind;
  }
}

export function codexTimelineSummary(kind, item) {
  switch (kind) {
    case "reasoning":
      return shortText(item.text || item.summary?.join?.("\n"), 1200) || "";
    case "command":
      return shortText(item.command, 1200) || "";
    case "file_change":
      return summarizeCodexFileChanges(item.changes);
    case "mcp":
      return [item.server, item.tool].filter(Boolean).join(" / ");
    case "search":
      return shortText(item.query, 1200) || "";
    case "todo_list":
      return summarizeCodexTodoList(item.items);
    case "error":
      return shortText(item.message, 1200) || "";
    default:
      return "";
  }
}

export function codexTimelinePayload(kind, subkind, item, eventType) {
  const base = {
    backend: "codex",
    eventType,
    itemType: getCodexItemType(item),
    status: item?.status,
  };
  if (subkind) base.subkind = subkind;
  switch (kind) {
    case "reasoning":
      return { ...base, text: item.text, summary: item.summary, content: item.content };
    case "command":
      return {
        ...base,
        command: item.command,
        output: item.aggregatedOutput,
        exit: item.exitCode,
      };
    case "file_change":
      return { ...base, changes: item.changes };
    case "mcp":
      return { ...base, server: item.server, tool: item.tool };
    case "search":
      return { ...base, query: item.query };
    case "todo_list":
      return { ...base, items: item.items };
    case "error":
      return { ...base, message: item.message };
    default:
      return base;
  }
}

export function maybeEmitCodexPlanApproval(eventType, item, ctx) {
  if (!ctx?.planMode || ctx.planApprovalHandled) return false;
  if (getCodexItemType(item) !== "plan") return false;
  const items = Array.isArray(item?.items) ? item.items : [];
  if (items.length === 0 || items.every((todo) => todo?.completed === true)) return false;
  ctx.planApprovalHandled = true;
  ctx.pendingPlanPayload = {
    source: "Codex plan",
    plan: summarizeCodexTodoList(items),
    approved: null,
    executionPermission: ctx.executionPermission,
    items,
  };
  ctx.protocol.emitTimeline({
    kind: "plan",
    status: "requires_action",
    title: "Codex plan",
    summary: oneLineSummary(ctx.pendingPlanPayload.plan),
    payload: {
      backend: "codex",
      ...ctx.pendingPlanPayload,
    },
    sourceId: item?.id || "codex:plan",
  });
  return true;
}

export function emitCodexPlanResolution(ctx, result) {
  const pending = ctx?.pendingPlanPayload;
  if (!pending) return;
  const revisionRequest = readPlanRevisionRequest(result);
  const approved = revisionRequest ? false : isPlanApprovalAccepted(result);
  ctx.protocol.emitTimeline({
    kind: "plan",
    status: approved ? "success" : "cancelled",
    title: "Codex plan",
    summary: oneLineSummary(revisionRequest || pending.plan),
    payload: {
      backend: "codex",
      ...pending,
      approved,
      ...(revisionRequest ? { revisionRequest } : {}),
    },
    sourceId: "codex:plan",
  });
}

export function emitCodexItemTimeline(eventType, item, ctx = null) {
  if (maybeEmitCodexPlanApproval(eventType, item, ctx)) {
    ctx.protocol.emit({ type: "todo_list", items: item.items });
    return;
  }
  const route = codexTimelineKindForItem(item);
  if (!route) return;
  const { kind, subkind = null } = route;
  ctx.protocol.emitTimeline({
    kind,
    status: getCodexStatus(eventType, item),
    title: codexTimelineTitle(kind, item, eventType),
    summary: codexTimelineSummary(kind, item),
    payload: codexTimelinePayload(kind, subkind, item, eventType),
    sourceId: item?.id,
  });
  if (kind === "todo_list" && Array.isArray(item?.items)) {
    ctx.protocol.emit({ type: "todo_list", items: item.items });
  }
}

export function emitCodexTurnTimeline(eventType, ev, ctx) {
  const errorMessage = ev?.error?.message || ev?.message || "";
  ctx.protocol.emitTimeline({
    kind: "turn",
    status: getCodexStatus(eventType, null),
    title:
      eventType === "turn.started"
        ? "Codex turn started"
        : eventType === "turn.completed"
          ? "Codex turn completed"
          : "Codex turn failed",
    summary: errorMessage || "",
    payload: {
      backend: "codex",
      eventType,
      usage: ev?.usage,
      error: ev?.error,
      sessionId: ctx?.lastThreadId,
    },
  });
}

export function mapCodexEventToNdjson(ev, ctx) {
  if (!ev || typeof ev !== "object") return;

  const tid = ev.threadId;
  if (tid && typeof tid === "string") ctx.lastThreadId = tid;

  const type = ev.type || "";

  if (type === "agentMessage.delta") {
    const delta = stringOrNull(ev.delta) || "";
    if (delta) {
      ctx.assistantDeltaText += delta;
      ctx.assistantSnapshotText += delta;
      ctx.pacer.push(delta);
    }
    return;
  }

  if (type === "thread.started") return;

  if (type === "turn.started") {
    emitCodexTurnTimeline(type, ev, ctx);
    return;
  }

  if (type === "item.completed" || type === "item.started") {
    const item = ev.item || ev;
    const kind = item?.type;
    emitCodexItemTimeline(type, item, ctx);
    if (kind === "agentMessage") {
      const text = pickCodexAssistantText(item);
      if (text && type === "item.completed") {
        ctx.assistantSnapshotText = text;
        emitCodexAssistantSuccess(ctx, text);
      }
      return;
    }
    if (type === "item.started") {
      const name = String(kind || "tool");
      const { type: _ignore, ...rest } = item || {};
      ctx.protocol.emit({ type: "tool_use", name, input: rest });
    }
    return;
  }

  if (type === "turn.completed") {
    ctx.turnCompletedSeen = true;
    emitCodexAssistantSuccess(ctx);
    emitCodexTurnTimeline(type, ev, ctx);
    ctx.protocol.emit({
      type: "done",
      sessionId: ctx.lastThreadId,
      subtype: "success",
    });
    return;
  }

  if (type === "turn.failed" || type === "error") {
    const msg = ev.error?.message || ev.message || "codex turn failed";
    ctx.turnCompletedSeen = true;
    ctx.pacer.finishImmediate();
    emitCodexTurnTimeline(type, ev, ctx);
    ctx.protocol.emit({ type: "error", message: msg });
  }
}

export function pickCodexAssistantText(item) {
  if (!item) return "";
  if (typeof item.text === "string") return item.text;
  if (typeof item.content === "string") return item.content;
  if (Array.isArray(item.content)) {
    return item.content
      .filter((b) => b && (b.type === "text" || b.type === "output_text"))
      .map((b) => (typeof b.text === "string" ? b.text : ""))
      .join("");
  }
  return "";
}

export function normalizeCodexAppServerEvent(msg) {
  if (!isRecord(msg)) return null;
  const method = stringOrNull(msg.method);
  const params = isRecord(msg.params) ? msg.params : {};
  if (method === "turn/started") return { type: "turn.started", ...params };
  if (method === "turn/completed") {
    const turn = isRecord(params.turn) ? params.turn : null;
    const status = stringOrNull(turn?.status);
    if (status === "failed" || status === "interrupted") {
      return { type: "turn.failed", ...params, error: turn?.error || params.error };
    }
    return { type: "turn.completed", ...params };
  }
  if (method === "turn/failed") return { type: "turn.failed", ...params };
  if (method === "turn/plan/updated") {
    return {
      type: "item.started",
      item: {
        id: stringOrNull(params.turnId) || "codex:plan",
        type: "plan",
        items: normalizeCodexPlanSteps(params.plan),
      },
    };
  }
  if (method === "item/started") return { type: "item.started", item: params.item || params };
  if (method === "item/completed") return { type: "item.completed", item: params.item || params };
  if (method === "item/agentMessage/delta") {
    return {
      type: "agentMessage.delta",
      id: stringOrNull(params.itemId) || "codex:agent-message",
      delta: stringOrNull(params.delta) || "",
    };
  }
  return null;
}

export function normalizeCodexPlanSteps(plan) {
  return Array.isArray(plan)
    ? plan
      .map((step) => {
        if (!isRecord(step)) return null;
        const text = stringOrNull(step.step) || stringOrNull(step.text);
        if (!text) return null;
        return {
          text,
          completed: step.status === "completed",
          status: stringOrNull(step.status),
        };
      })
      .filter(Boolean)
    : [];
}

export function createCodexRunContext(cmd, protocol, sessionId = null) {
  return {
    protocol,
    lastThreadId: sessionId,
    assistantDeltaText: "",
    assistantSnapshotText: "",
    turnCompletedSeen: false,
    planMode: cmd.planMode === true,
    planApprovalHandled: false,
    planApprovalResolved: false,
    pendingPlanPayload: null,
    executionPermission: cmd.permission === "full" || cmd.permission === "readonly" ? cmd.permission : "ask",
    assistantSuccessEmitted: false,
    pacer: createTextPacer({
      emit: (text) => protocol.emitAssistantMessageTimeline(text, "running", "codex"),
    }),
  };
}

export function emitCodexAssistantSuccess(ctx, finalText = null) {
  if (!ctx || ctx.assistantSuccessEmitted) return;
  const text =
    fullTextOrNull(finalText) ||
    fullTextOrNull(ctx.assistantSnapshotText) ||
    fullTextOrNull(ctx.assistantDeltaText);
  ctx.pacer.finishImmediate();
  if (!text) return;
  ctx.assistantSuccessEmitted = true;
  ctx.protocol.emitAssistantMessageTimeline(text, "success", "codex");
}

export async function maybeHandleCodexPlanApproval(ctx) {
  if (!ctx?.pendingPlanPayload || ctx.planApprovalResolved) return;
  ctx.planApprovalResolved = true;
  const result = await ctx.interactions.requestAskUser(buildPlanApprovalSpec(), { emitTimelineEvent: false });
  emitCodexPlanResolution(ctx, result);
}

export function finalizeCodexRunContext(ctx) {
  if (!ctx.lastThreadId || ctx.turnCompletedSeen) return;
  const finalText =
    fullTextOrNull(ctx.assistantSnapshotText) ||
    fullTextOrNull(ctx.assistantDeltaText);
  emitCodexAssistantSuccess(ctx, finalText);
  ctx.protocol.emitTimeline({
    kind: "turn",
    status: "success",
    title: "Codex turn completed",
    summary: "",
    payload: {
      backend: "codex",
      eventType: "turn.completed",
      sessionId: ctx.lastThreadId,
    },
  });
  ctx.protocol.emit({ type: "done", sessionId: ctx.lastThreadId, subtype: "success" });
}
