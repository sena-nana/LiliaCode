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
import { isRecord, stringOrNull } from "../utils.mjs";
import { createCodexAppServer } from "./appServer.mjs";
import {
  mapCodexApprovalPolicy,
  mapCodexSandboxMode,
  maybeHandleCodexApprovalRequest,
} from "./permissions.mjs";
import {
  createCodexRunContext,
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

export async function startCodexAppServerSession(server, cmd, cwdFn = process.cwd) {
  const { cwd, model, resumeSessionId, permission } = cmd;
  const common = {
    model: model || undefined,
    cwd: cwd || cwdFn(),
    sandbox: mapCodexSandboxMode(permission),
    approvalPolicy: mapCodexApprovalPolicy(permission),
  };
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
  const params = {
    threadId,
    input: [{ type: "text", text: prompt }],
    cwd: cmd.cwd || cwdFn(),
    approvalPolicy: mapCodexApprovalPolicy(cmd.permission),
  };
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
    const selectedModel = cmd.model || session.model || null;
    const planPreset = cmd.planMode === true ? await readCodexPlanModePreset(server) : null;
    const ctx = createCodexRunContext(cmd, context.protocol, threadId);
    ctx.interactions = context.interactions;
    ctx.emitToolConsentTimeline = context.emitToolConsentTimeline;
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
    kind: "mcp",
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
