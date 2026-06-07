import {
  askUserResultToCodexRequestUserInputResponse,
  codexAskUserDynamicTool,
  codexRequestUserInputQuestionsToSpec,
  createCodexAskUserHandler,
  isLiliaAskUserTool,
} from "../askUser.mjs";
import {
  emitRuntimeExtensionWarnings,
  readCodexRuntimeExtensions,
} from "../runtimeExtensions.mjs";
import { normalizeRuntimePermission } from "../runtimeSettings.mjs";
import { isRecord, stringOrNull } from "../utils.mjs";
import { createCodexAppServer } from "./appServer.mjs";
import {
  mapCodexApprovalPolicy,
  mapCodexSandboxMode,
  maybeHandleCodexApprovalRequest,
} from "./permissions.mjs";
import {
  createCodexRunContext,
  codexHistoryTimelineEvents,
  emitCodexPlanApprovalRequired,
  finalizeCodexRunContext,
  mapCodexEventToNdjson,
  normalizeCodexAppServerEvent,
  resetCodexContextForNextTurn,
  resolveCodexPlanApproval,
} from "./timeline.mjs";
import { buildPlanApprovalSpec } from "../planApproval.mjs";

export async function initializeCodexAppServer(server) {
  await server.request("initialize", {
    clientInfo: {
      name: "lilia",
      title: "LiliaCode",
      version: "0.1.0",
    },
    capabilities: { experimentalApi: true },
  });
  server.notify("initialized", {});
}

function codexThreadIdFromResult(result, fallback = null) {
  if (!isRecord(result)) return fallback;
  return stringOrNull(result.thread?.id) || stringOrNull(result.threadId) || fallback;
}

function codexModelFromResult(result, fallback = null) {
  if (!isRecord(result)) return fallback;
  return stringOrNull(result.model) || stringOrNull(result.thread?.model) || fallback;
}

const CODEX_PERMISSION_PROFILE_IDS = {
  readOnly: ":read-only",
  workspaceWrite: ":workspace",
  dangerFullAccess: ":danger-no-sandbox",
};

function stringArray(value) {
  if (!Array.isArray(value)) return [];
  const seen = new Set();
  const out = [];
  for (const item of value) {
    const text = stringOrNull(item)?.trim();
    if (!text || seen.has(text)) continue;
    seen.add(text);
    out.push(text);
  }
  return out;
}

function normalizeCodexSettings(cmd) {
  const input = isRecord(cmd.codexSettings) ? cmd.codexSettings : {};
  const permissions = isRecord(input.permissions) ? input.permissions : {};
  const permissionProfile = stringOrNull(permissions.profile);
  return {
    profile: stringOrNull(input.profile) || "default",
    model: stringOrNull(input.model) || stringOrNull(cmd.model) || null,
    reasoningEffort: stringOrNull(input.reasoningEffort),
    runtimeWorkspaceRoots: stringArray(input.runtimeWorkspaceRoots),
    permissionProfile: CODEX_PERMISSION_PROFILE_IDS[permissionProfile] || null,
  };
}

function codexSandboxPolicy(permission) {
  if (permission === "full") return { type: "dangerFullAccess" };
  if (permission === "readonly") return { type: "readOnly" };
  return { type: "workspaceWrite" };
}

function assignCodexSettingsParams(params, settings, cmd, { includeSandbox = false } = {}) {
  if (settings.model) params.model = settings.model;
  if (settings.reasoningEffort) {
    params.reasoningEffort = settings.reasoningEffort;
    params.effort = settings.reasoningEffort;
  }
  if (settings.runtimeWorkspaceRoots.length > 0) {
    params.runtimeWorkspaceRoots = settings.runtimeWorkspaceRoots;
  }
  if (settings.permissionProfile) {
    params.permissions = settings.permissionProfile;
  } else if (includeSandbox) {
    params.sandboxPolicy = codexSandboxPolicy(cmd.permission);
  }
}

function buildCodexThreadSettingsParams(threadId, cmd) {
  const settings = normalizeCodexSettings(cmd);
  const params = {
    threadId,
    approvalPolicy: mapCodexApprovalPolicy(cmd.permission),
  };
  assignCodexSettingsParams(params, settings, cmd, { includeSandbox: true });
  return params;
}

function emitCodexSettingsUpdateDiagnostic(protocol, err, params) {
  protocol.emitTimeline({
    kind: "diagnostic",
    status: "error",
    title: "Codex settings update failed",
    summary: "thread/settings/update 失败，已降级为本轮 turn/start 参数。",
    payload: {
      backend: "codex",
      subkind: "settings",
      method: "thread/settings/update",
      error: err?.message || String(err),
      fallback: "turn/start",
      settingsKeys: Object.keys(params).filter((key) => key !== "threadId"),
    },
    sourceId: "codex:settings:update",
  });
}

export async function updateCodexThreadSettings(server, threadId, cmd, protocol) {
  const params = buildCodexThreadSettingsParams(threadId, cmd);
  try {
    await server.request("thread/settings/update", params);
    return { ok: true, fallback: false };
  } catch (err) {
    emitCodexSettingsUpdateDiagnostic(protocol, err, params);
    return { ok: false, fallback: true };
  }
}

export function applyCodexRuntimePermission(server, threadId, cmd, ctx, permission, protocol) {
  const normalized = normalizeRuntimePermission(permission);
  if (!normalized) return null;
  cmd.permission = normalized;
  ctx.executionPermission = normalized;
  const pending = updateCodexThreadSettings(server, threadId, cmd, protocol)
    .then((result) => {
      if (!result.ok) return result;
      protocol.emitTimeline({
        kind: "diagnostic",
        status: "info",
        title: "Codex permission updated",
        summary: `权限已切换为 ${normalized}`,
        payload: {
          backend: "codex",
          subkind: "settings",
          permission: normalized,
          fallback: false,
        },
        sourceId: "codex:settings:permission",
      });
      return result;
    });
  ctx.settingsUpdatePromises.push(pending);
  return pending;
}

export async function flushCodexRuntimeSettings(ctx) {
  if (ctx.settingsUpdatePromises.length === 0) return;
  const pending = ctx.settingsUpdatePromises.splice(0);
  await Promise.allSettled(pending);
}

export async function startCodexAppServerSession(server, cmd, cwdFn = process.cwd) {
  const { cwd, model, resumeSessionId, permission } = cmd;
  const settings = normalizeCodexSettings(cmd);
  const common = {
    model: settings.model || model || undefined,
    cwd: cwd || cwdFn(),
    approvalPolicy: mapCodexApprovalPolicy(permission),
  };
  assignCodexSettingsParams(common, settings, cmd);
  if (!settings.permissionProfile) common.sandbox = mapCodexSandboxMode(permission);
  if (resumeSessionId) {
    const resumed = await server.request("thread/resume", {
      threadId: resumeSessionId,
      ...common,
      dynamicTools: [codexAskUserDynamicTool],
    });
    return {
      threadId: codexThreadIdFromResult(resumed, resumeSessionId),
      model: codexModelFromResult(resumed, model || null),
    };
  }
  const started = await server.request("thread/start", {
    ...common,
    dynamicTools: [codexAskUserDynamicTool],
  });
  return {
    threadId: codexThreadIdFromResult(started),
    model: codexModelFromResult(started, model || null),
  };
}

export async function startCodexAppServerThread(server, cmd, cwdFn = process.cwd) {
  return (await startCodexAppServerSession(server, cmd, cwdFn)).threadId;
}

export function buildCodexPlanRevisionPrompt(revisionRequest) {
  return [
    "用户要求修改上一版计划，暂不执行当前计划。",
    `修改要求：${revisionRequest}`,
    "请根据这条修改要求更新计划，并再次等待 Lilia 的计划确认；在计划确认前不要执行文件修改或命令。",
  ].join("\n");
}

export function buildCodexPlanExecutionPrompt(plan) {
  return [
    "用户已确认上一版计划。",
    "请按以下已确认计划继续执行；不要再次请求计划确认，除非用户提出新的计划修改要求。",
    "",
    "已确认计划：",
    plan,
  ].join("\n");
}

function codexTurnIdFromStartResult(result) {
  if (!isRecord(result)) return null;
  const turn = isRecord(result.turn) ? result.turn : null;
  return stringOrNull(turn?.id) || stringOrNull(result.turnId);
}

function readCollaborationModes(result) {
  if (!isRecord(result)) return [];
  return Array.isArray(result.data) ? result.data.filter(isRecord) : [];
}

export async function readCodexPlanModePreset(server) {
  try {
    const result = await server.request("collaborationMode/list", {});
    return readCollaborationModes(result).find((mode) => mode.mode === "plan") || null;
  } catch (err) {
    return null;
  }
}

function readCodexTurns(result) {
  if (!isRecord(result)) return [];
  return Array.isArray(result.data) ? result.data.filter(isRecord) : [];
}

export async function syncCodexThreadHistory(server, threadId, cmd, protocol) {
  if (!cmd.resumeSessionId || !threadId) return { ok: true, skipped: true, count: 0 };
  try {
    const result = await server.request("thread/turns/list", {
      threadId,
      limit: 50,
      sortDirection: "asc",
      itemsView: "full",
    });
    const events = codexHistoryTimelineEvents(threadId, readCodexTurns(result));
    for (const event of events) protocol.emitTimeline(event);
    protocol.emitTimeline({
      kind: "diagnostic",
      status: "info",
      title: "Codex history synced",
      summary: events.length > 0
        ? `已同步 ${events.length} 条 Codex 历史事件`
        : "没有需要同步的 Codex 历史事件",
      payload: {
        backend: "codex",
        subkind: "history_sync",
        threadId,
        eventCount: events.length,
        nextCursor: isRecord(result) ? result.nextCursor ?? null : null,
      },
      sourceId: `codex-history:${threadId}:sync`,
      turnIdOverride: `codex-history:${threadId}`,
    });
    return { ok: true, skipped: false, count: events.length };
  } catch (err) {
    protocol.emitTimeline({
      kind: "diagnostic",
      status: "error",
      title: "Codex history sync failed",
      summary: "thread/turns/list 失败，已跳过历史回补并继续当前 turn。",
      payload: {
        backend: "codex",
        subkind: "history_sync",
        method: "thread/turns/list",
        threadId,
        error: err?.message || String(err),
      },
      sourceId: `codex-history:${threadId}:sync-error`,
      turnIdOverride: `codex-history:${threadId}`,
    });
    return { ok: false, skipped: false, count: 0 };
  }
}

export function buildCodexCollaborationMode(kind, model, preset = null) {
  const fallbackModel = stringOrNull(model) || stringOrNull(preset?.model) || "gpt-5";
  return {
    mode: kind === "plan" ? "plan" : "default",
    settings: {
      model: fallbackModel,
      reasoning_effort:
        kind === "plan"
          ? stringOrNull(preset?.reasoning_effort) || "medium"
          : null,
      developer_instructions: null,
    },
  };
}

export async function startCodexAppServerTurn(
  server,
  threadId,
  prompt,
  cmd,
  cwdFn = process.cwd,
  options = {},
) {
  const settings = normalizeCodexSettings(cmd);
  const params = {
    threadId,
    input: [{ type: "text", text: prompt }],
    cwd: cmd.cwd || cwdFn(),
    approvalPolicy: mapCodexApprovalPolicy(cmd.permission),
  };
  assignCodexSettingsParams(params, settings, cmd, { includeSandbox: true });
  if (options.collaborationMode) params.collaborationMode = options.collaborationMode;
  return server.request("turn/start", params);
}

export async function maybeHandleCodexServerRequest(server, msg, ctx = null) {
  if (!isRecord(msg)) return false;
  const method = stringOrNull(msg.method);
  if (method === "item/tool/call") {
    const params = isRecord(msg.params) ? msg.params : {};
    const toolName = stringOrNull(params.tool);
    if (!isLiliaAskUserTool(toolName)) return false;
    const input = isRecord(params.arguments) ? params.arguments : {};
    const output = await createCodexAskUserHandler(ctx.interactions.requestAskUser)(input);
    server.respond(msg.id, {
      success: output.cancelled !== true,
      contentItems: [{ type: "inputText", text: JSON.stringify(output) }],
    });
    return true;
  }
  if (method === "item/tool/requestUserInput") {
    const params = isRecord(msg.params) ? msg.params : {};
    const spec = codexRequestUserInputQuestionsToSpec(params);
    if (spec.questions.length === 0) {
      server.respond(msg.id, { answers: {} });
      return true;
    }
    const result = await ctx.interactions.requestAskUser(spec, { backend: "codex" });
    server.respond(msg.id, askUserResultToCodexRequestUserInputResponse(result, spec));
    return true;
  }
  return maybeHandleCodexApprovalRequest(server, msg, ctx);
}

export async function runCodexAppServer(cmd, runtimeExtensions, context) {
  const server = context.createCodexAppServer
    ? context.createCodexAppServer()
    : createCodexAppServer({ env: context.env || process.env });
  emitCodexRuntimeExtensionsTimeline(context.protocol, runtimeExtensions);
  try {
    await initializeCodexAppServer(server);
    const session = await startCodexAppServerSession(server, cmd, context.cwd || process.cwd);
    const threadId = session.threadId;
    if (!threadId) throw new Error("Codex app-server did not return a thread id");
    await updateCodexThreadSettings(
      server,
      threadId,
      cmd,
      context.protocol,
    );
    await syncCodexThreadHistory(server, threadId, cmd, context.protocol);
    const selectedModel = normalizeCodexSettings(cmd).model || session.model || null;
    const planPreset = cmd.planMode === true ? await readCodexPlanModePreset(server) : null;
    const ctx = createCodexRunContext(cmd, context.protocol, threadId);
    ctx.interactions = context.interactions;
    ctx.emitToolConsentTimeline = context.emitToolConsentTimeline;
    ctx.settingsUpdatePromises = [];
    context.interactions?.handleSettingsUpdate?.((update) => {
      applyCodexRuntimePermission(
        server,
        threadId,
        cmd,
        ctx,
        update?.permission,
        context.protocol,
      );
    });
    let nextPrompt = cmd.prompt;
    let nextTurnKind = cmd.planMode === true ? "plan" : "default";
    let shouldStartTurn = true;
    while (shouldStartTurn) {
      shouldStartTurn = false;
      ctx.activeTurnKind = nextTurnKind;
      const collaborationMode =
        nextTurnKind === "plan"
          ? buildCodexCollaborationMode("plan", selectedModel, planPreset)
          : buildCodexCollaborationMode("default", selectedModel, null);
      await flushCodexRuntimeSettings(ctx);
      const startedTurn = await startCodexAppServerTurn(
        server,
        threadId,
        nextPrompt,
        cmd,
        context.cwd || process.cwd,
        { collaborationMode },
      );
      ctx.currentTurnId = codexTurnIdFromStartResult(startedTurn) || ctx.currentTurnId;
      const startedAt = Date.now();
      while (!ctx.turnCompletedSeen) {
        const messages = server.drainNotifications();
        for (const msg of messages) {
          if (await maybeHandleCodexServerRequest(server, msg, ctx)) continue;
          const ev = normalizeCodexAppServerEvent(msg);
          if (ev) mapCodexEventToNdjson(ev, ctx);
        }
        if (Date.now() - startedAt > 1000 * 60 * 60) {
          throw new Error("Codex app-server turn timed out");
        }
        if (!ctx.turnCompletedSeen) {
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
      }
      if (ctx.turnFailedSeen) {
        continue;
      }
      if (nextTurnKind === "plan") {
        if (!emitCodexPlanApprovalRequired(ctx)) {
          throw new Error("Codex plan mode completed without a readable plan");
        }
        const result = await ctx.interactions.requestAskUser(buildPlanApprovalSpec("codex"), {
          backend: "codex",
          emitTimelineEvent: false,
        });
        resolveCodexPlanApproval(ctx, result);
        if (ctx.planRevisionRequest) {
          const revisionRequest = ctx.planRevisionRequest;
          resetCodexContextForNextTurn(ctx);
          nextPrompt = buildCodexPlanRevisionPrompt(revisionRequest);
          nextTurnKind = "plan";
          shouldStartTurn = true;
        } else if (ctx.planApproved) {
          const approvedPlan = ctx.pendingPlanPayload?.plan || "";
          resetCodexContextForNextTurn(ctx, { planMode: false });
          nextPrompt = buildCodexPlanExecutionPrompt(approvedPlan);
          nextTurnKind = "default";
          shouldStartTurn = true;
        } else if (ctx.planCancelled) {
          ctx.protocol.emit({
            type: "done",
            sessionId: ctx.lastThreadId,
            subtype: "cancelled",
          });
        }
      }
    }
    if (ctx.planCancelled) {
      return;
    }
    finalizeCodexRunContext(ctx);
  } finally {
    server.close();
  }
}

export async function runCodex(cmd, context) {
  const runtimeExtensions = readCodexRuntimeExtensions(cmd);
  await runCodexAppServer(cmd, runtimeExtensions, context);
}

export function emitCodexRuntimeExtensionsTimeline(protocol, extensions) {
  const count = extensions.mcpServers.length;
  const serverNames = extensions.mcpServers
    .map((server) => stringOrNull(server?.name))
    .filter(Boolean);
  protocol.emitTimeline({
    kind: "diagnostic",
    status: "info",
    title: "Codex MCP config",
    summary: count > 0
      ? `已注册 ${count} 个 MCP server`
      : "未发现 Codex MCP server",
    payload: {
      backend: "codex",
      subkind: "config",
      source: "config.toml",
      configPath: extensions.configPath,
      serverCount: count,
      servers: serverNames,
      warnings: extensions.warnings,
    },
    sourceId: "codex:mcp:runtime-config",
  });
  emitRuntimeExtensionWarnings(protocol, "codex", extensions.warnings);
}
