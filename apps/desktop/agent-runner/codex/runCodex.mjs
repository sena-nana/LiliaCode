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
  finalizeCodexRunContext,
  mapCodexEventToNdjson,
  maybeHandleCodexPlanApproval,
  normalizeCodexAppServerEvent,
} from "./timeline.mjs";

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

export async function startCodexAppServerThread(server, cmd, cwdFn = process.cwd) {
  const { cwd, model, resumeSessionId, permission, planMode } = cmd;
  const common = {
    model: model || undefined,
    cwd: cwd || cwdFn(),
    sandbox: mapCodexSandboxMode(permission),
    approvalPolicy: mapCodexApprovalPolicy(permission),
    includePlanTool: planMode === true,
  };
  if (resumeSessionId) {
    const resumed = await server.request("thread/resume", { threadId: resumeSessionId, ...common });
    return resumed?.thread?.id || resumed?.threadId || resumeSessionId;
  }
  const started = await server.request("thread/start", {
    ...common,
    dynamicTools: [codexAskUserDynamicTool],
  });
  return started?.thread?.id || started?.threadId || null;
}

export async function startCodexAppServerTurn(server, threadId, prompt, cmd, cwdFn = process.cwd) {
  return server.request("turn/start", {
    threadId,
    input: [{ type: "text", text: prompt }],
    cwd: cmd.cwd || cwdFn(),
    approvalPolicy: mapCodexApprovalPolicy(cmd.permission),
  });
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
    const threadId = await startCodexAppServerThread(server, cmd, context.cwd || process.cwd);
    if (!threadId) throw new Error("Codex app-server did not return a thread id");
    const ctx = createCodexRunContext(cmd, context.protocol, threadId);
    ctx.interactions = context.interactions;
    ctx.emitToolConsentTimeline = context.emitToolConsentTimeline;
    await startCodexAppServerTurn(server, threadId, cmd.prompt, cmd, context.cwd || process.cwd);
    const startedAt = Date.now();
    while (!ctx.turnCompletedSeen) {
      const messages = server.drainNotifications();
      for (const msg of messages) {
        if (await maybeHandleCodexServerRequest(server, msg, ctx)) continue;
        const ev = normalizeCodexAppServerEvent(msg);
        if (ev) mapCodexEventToNdjson(ev, ctx);
        await maybeHandleCodexPlanApproval(ctx);
      }
      if (Date.now() - startedAt > 1000 * 60 * 60) {
        throw new Error("Codex app-server turn timed out");
      }
      await new Promise((resolve) => setTimeout(resolve, 20));
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
