import {
  askUserResultToCodexRequestUserInputResponse,
  codexAskUserDynamicTool,
  codexRequestUserInputQuestionsToSpec,
  createCodexAskUserHandler,
  isLiliaAskUserTool,
} from "../askUser.mjs";
import {
  codexQueryConversationContextDynamicTool,
  conversationContextEnabled,
  createConversationContextHandler,
  isLiliaConversationContextTool,
} from "../conversationContext.mjs";
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

function emitCodexElicitationDiagnostic(ctx, method, kind, err) {
  ctx?.protocol?.emitTimeline?.({
    kind: "diagnostic",
    status: "error",
    title: "Codex elicitation update failed",
    summary: `${method} 失败，Lilia 已继续等待用户交互。`,
    payload: {
      backend: "codex",
      subkind: "elicitation",
      method,
      interactionKind: kind,
      error: err?.message || String(err),
    },
    sourceId: `codex:elicitation:${method}:${kind}`,
  });
}

export async function withCodexElicitation(server, threadId, ctx, kind, fn) {
  if (typeof fn !== "function") return undefined;
  if (!server || !threadId) return await fn();
  let incremented = false;
  try {
    await server.request("thread/increment_elicitation", { threadId });
    incremented = true;
  } catch (err) {
    emitCodexElicitationDiagnostic(ctx, "thread/increment_elicitation", kind, err);
  }
  try {
    return await fn();
  } finally {
    if (incremented) {
      try {
        await server.request("thread/decrement_elicitation", { threadId });
      } catch (err) {
        emitCodexElicitationDiagnostic(ctx, "thread/decrement_elicitation", kind, err);
      }
    }
  }
}

async function runCodexUiInteraction(ctx, kind, fn) {
  return ctx?.withCodexElicitation
    ? ctx.withCodexElicitation(kind, fn)
    : await fn();
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
  const dynamicTools = [codexAskUserDynamicTool];
  if (conversationContextEnabled(cmd.conversationContext)) {
    dynamicTools.push(codexQueryConversationContextDynamicTool);
  }
  if (resumeSessionId) {
    const resumed = await server.request("thread/resume", {
      threadId: resumeSessionId,
      ...common,
      dynamicTools,
    });
    return {
      threadId: codexThreadIdFromResult(resumed, resumeSessionId),
      model: codexModelFromResult(resumed, model || null),
    };
  }
  const started = await server.request("thread/start", {
    ...common,
    dynamicTools,
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

function readCodexWorkflow(cmd) {
  return isRecord(cmd?.workflow) ? cmd.workflow : null;
}

function normalizeCodexReviewTarget(target) {
  if (!isRecord(target)) return null;
  const type = stringOrNull(target.type);
  if (type === "uncommittedChanges") return { type };
  if (type === "baseBranch") {
    const branch = stringOrNull(target.branch)?.trim();
    return branch ? { type, branch } : null;
  }
  if (type === "commit") {
    const sha = stringOrNull(target.sha)?.trim();
    return sha ? { type, sha, title: null } : null;
  }
  return null;
}

const CODEX_GOAL_ACTIONS = new Set(["set", "refresh", "clear"]);
const CODEX_GOAL_STATUSES = new Set([
  "active",
  "paused",
  "blocked",
  "usageLimited",
  "budgetLimited",
  "complete",
]);

function readCodexReviewWorkflow(cmd) {
  const workflow = readCodexWorkflow(cmd);
  if (workflow?.type !== "codex_review") return null;
  const target = normalizeCodexReviewTarget(workflow.target);
  if (!target) throw new Error("Codex review workflow missing a valid target");
  const instructions = stringOrNull(workflow.instructions)?.trim() || "";
  const delivery = workflow.delivery === "detached" ? "detached" : "inline";
  return { target, instructions, delivery };
}

function normalizeCodexGoalStatus(status) {
  const value = stringOrNull(status);
  return CODEX_GOAL_STATUSES.has(value) ? value : null;
}

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readCodexGoalWorkflow(cmd) {
  const workflow = readCodexWorkflow(cmd);
  if (workflow?.type !== "codex_goal") return null;
  const action = stringOrNull(workflow.action);
  if (!CODEX_GOAL_ACTIONS.has(action)) {
    throw new Error("Codex goal workflow missing a valid action");
  }
  const objective = stringOrNull(workflow.objective)?.trim() || "";
  if (action === "set" && !objective) {
    throw new Error("Codex goal workflow missing objective");
  }
  const tokenBudget = numberOrNull(workflow.tokenBudget);
  return {
    action,
    objective,
    status: normalizeCodexGoalStatus(workflow.status) || "active",
    tokenBudget,
  };
}

function codexReviewTargetSummary(target) {
  if (target.type === "baseBranch") return `baseBranch:${target.branch}`;
  if (target.type === "commit") return `commit:${target.sha}`;
  return "uncommittedChanges";
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

export async function startCodexReview(server, threadId, review) {
  const params = {
    threadId,
    target: review.target,
    delivery: review.delivery,
  };
  if (review.instructions) params.prompt = review.instructions;
  return server.request("review/start", params);
}

export async function runCodexGoal(server, threadId, goal) {
  if (goal.action === "clear") {
    await server.request("thread/goal/clear", { threadId });
    return { action: goal.action, goal: null };
  }
  if (goal.action === "refresh") {
    const result = await server.request("thread/goal/get", { threadId });
    return { action: goal.action, goal: isRecord(result?.goal) ? result.goal : null };
  }
  const params = {
    threadId,
    objective: goal.objective,
    status: goal.status,
    tokenBudget: goal.tokenBudget ?? null,
  };
  const result = await server.request("thread/goal/set", params);
  return { action: goal.action, goal: isRecord(result?.goal) ? result.goal : null };
}

async function drainCodexTurnNotifications(server, ctx, timeoutMessage) {
  const startedAt = Date.now();
  while (!ctx.turnCompletedSeen) {
    const messages = server.drainNotifications();
    for (const msg of messages) {
      if (await maybeHandleCodexServerRequest(server, msg, ctx)) continue;
      const ev = normalizeCodexAppServerEvent(msg);
      if (ev) mapCodexEventToNdjson(ev, ctx);
    }
    if (Date.now() - startedAt > 1000 * 60 * 60) {
      throw new Error(timeoutMessage);
    }
    if (!ctx.turnCompletedSeen) {
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }
}

async function runCodexReviewWorkflow(server, threadId, cmd, ctx) {
  const review = readCodexReviewWorkflow(cmd);
  if (!review) return false;
  await flushCodexRuntimeSettings(ctx);
  const result = await startCodexReview(server, threadId, review);
  ctx.currentTurnId = codexTurnIdFromStartResult(result) || ctx.currentTurnId;
  ctx.protocol.emitTimeline({
    kind: "diagnostic",
    status: "started",
    title: "Codex review started",
    summary: codexReviewTargetSummary(review.target),
    payload: {
      backend: "codex",
      subkind: "review",
      method: "review/start",
      target: review.target,
      delivery: review.delivery,
      hasInstructions: Boolean(review.instructions),
    },
    sourceId: `codex:review:start:${ctx.currentTurnId || codexReviewTargetSummary(review.target)}`,
  });
  await drainCodexTurnNotifications(server, ctx, "Codex review timed out");
  return true;
}

function emitCodexGoalWorkflowTimeline(ctx, action, goal) {
  const cleared = action === "clear";
  ctx.protocol.emitTimeline({
    kind: "goal",
    status: cleared ? "cancelled" : "info",
    title: cleared ? "Codex goal cleared" : "Codex goal updated",
    summary: cleared
      ? "已清除 Codex thread goal"
      : stringOrNull(goal?.objective) || "Codex thread goal",
    payload: {
      backend: "codex",
      subkind: "thread_goal",
      action,
      cleared,
      goal: goal ?? null,
      goalStatus: stringOrNull(goal?.status),
      threadId: ctx.threadId,
    },
    sourceId: `codex:goal:${action}:${ctx.threadId}`,
  });
}

async function runCodexGoalWorkflow(server, threadId, cmd, ctx) {
  const goal = readCodexGoalWorkflow(cmd);
  if (!goal) return false;
  await flushCodexRuntimeSettings(ctx);
  const result = await runCodexGoal(server, threadId, goal);
  emitCodexGoalWorkflowTimeline(ctx, result.action, result.goal);
  return true;
}

export async function maybeHandleCodexServerRequest(server, msg, ctx = null) {
  if (!isRecord(msg)) return false;
  const method = stringOrNull(msg.method);
  if (method === "item/tool/call") {
    const params = isRecord(msg.params) ? msg.params : {};
    const toolName = stringOrNull(params.tool);
    const input = isRecord(params.arguments) ? params.arguments : {};
    let output = null;
    if (isLiliaAskUserTool(toolName)) {
      const requestAskUser = (spec, options) =>
        runCodexUiInteraction(ctx, "ask_user", () =>
          ctx.interactions.requestAskUser(spec, options));
      output = await createCodexAskUserHandler(requestAskUser)(input);
    } else if (isLiliaConversationContextTool(toolName)) {
      output = await createConversationContextHandler(ctx.cmd?.conversationContext)(input);
    } else {
      return false;
    }
    server.respond(msg.id, {
      success: output.cancelled !== true && output.ok !== false,
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
    const result = await runCodexUiInteraction(ctx, "ask_user", () =>
      ctx.interactions.requestAskUser(spec, { backend: "codex" }));
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
    ctx.cmd = cmd;
    ctx.interactions = context.interactions;
    ctx.emitToolConsentTimeline = context.emitToolConsentTimeline;
    ctx.settingsUpdatePromises = [];
    ctx.withCodexElicitation = (kind, fn) =>
      withCodexElicitation(server, threadId, ctx, kind, fn);
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
    if (await runCodexGoalWorkflow(server, threadId, cmd, ctx)) {
      finalizeCodexRunContext(ctx);
      return;
    }
    if (await runCodexReviewWorkflow(server, threadId, cmd, ctx)) {
      finalizeCodexRunContext(ctx);
      return;
    }
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
      await drainCodexTurnNotifications(server, ctx, "Codex app-server turn timed out");
      if (ctx.turnFailedSeen) {
        continue;
      }
      if (nextTurnKind === "plan") {
        if (!emitCodexPlanApprovalRequired(ctx)) {
          throw new Error("Codex plan mode completed without a readable plan");
        }
        const result = await runCodexUiInteraction(ctx, "plan_approval", () =>
          ctx.interactions.requestAskUser(buildPlanApprovalSpec("codex"), {
            backend: "codex",
            emitTimelineEvent: false,
          }));
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
