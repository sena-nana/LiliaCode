import { createSdkMcpServer, forkSession, query, tool } from "@anthropic-ai/claude-agent-sdk";
import { createClaudeStreamState, dispatchClaudeStreamEvent } from "../claudeStream.mjs";
import {
  askUserQuestionInputSchema,
  createClaudeAskUserHandler,
} from "../askUser.mjs";
import {
  architectureContextEnabled,
  createArchitectureChangeHandler,
  updateProjectArchitectureInputSchema,
} from "../architecture.mjs";
import {
  buildConversationContextToolDescription,
  conversationContextEnabled,
  createConversationContextHandler,
  queryConversationContextInputSchema,
} from "../conversationContext.mjs";
import {
  createQuotaUsageHandler,
  queryQuotaUsageInputSchema,
} from "../quotaUsage.mjs";
import {
  emitRuntimeExtensionWarnings,
  readClaudeRuntimeExtensions,
} from "../runtimeExtensions.mjs";
import {
  emitAssistantTextFragmentTimeline,
} from "../protocol.mjs";
import {
  applyClaudeRuntimePermission,
  mapClaudeInitialPermission,
  createClaudeCanUseTool,
  createClaudeHooks,
} from "./permissions.mjs";
import { normalizeRuntimePermission } from "../runtimeSettings.mjs";
import { runClaudeSessionManagementRuntimeCommand } from "../sessionManagement.mjs";
import { handleExperimentalProviderOptions } from "../providerOptions.mjs";
import {
  closeClaudeReasoningBlock,
  closeClaudeTextFragment,
  emitClaudePlanTimeline,
  emitClaudeTextResultFallback,
  emitClaudeToolResultTimeline,
  emitClaudeToolTimeline,
  finalizeClaudeReasoningTimeline,
  finalizeClaudeTextFragments,
  getOrCreateClaudeReasoningBlock,
  getOrCreateClaudeTextFragment,
  mapClaudeSystemTimeline,
  sweepActiveClaudeTools,
} from "./timeline.mjs";
import { isRecord, stringOrNull } from "../utils.mjs";

export function buildClaudePlatformAppend(platform = process.platform) {
  if (platform !== "win32") return "";
  return [
    "运行平台：Windows（win32）。",
    "Bash 工具在 Windows 上走 Git Bash 的 /usr/bin/bash，仅认 POSIX 命令，不认 PowerShell cmdlet（Get-ChildItem / Select-Object / Format-Table 等）。",
    "- 默认使用 POSIX 命令：ls / cat / grep / find / awk / sed / head / tail …",
    '- 必须用 PowerShell 时显式包装：powershell.exe -NoProfile -Command "<原 PS 命令> | Out-String"。',
    "- 路径用正斜杠或在双引号内用反斜杠都可。",
  ].join("\n");
}

export function buildClaudeSystemPrompt(platform = process.platform) {
  const append = buildClaudePlatformAppend(platform);
  const base = { type: "preset", preset: "claude_code" };
  return append ? { ...base, append } : base;
}

export async function* singleClaudePromptStream(prompt) {
  yield {
    type: "user",
    message: {
      role: "user",
      content: [{ type: "text", text: prompt }],
    },
    parent_tool_use_id: null,
  };
}

export function claudeCompactPromptStream() {
  return singleClaudePromptStream("/compact");
}

export function createLiliaAskUserServer({
  createServer = createSdkMcpServer,
  createTool = tool,
  requestAskUser,
  requestQuotaUsage,
  conversationContext = null,
  architectureHandler = null,
}) {
  const tools = [
    createTool(
      "ask_user_question",
      "Ask the human user one or more multiple-choice questions through Lilia.",
      askUserQuestionInputSchema,
      createClaudeAskUserHandler(requestAskUser),
      { alwaysLoad: true },
    ),
    createTool(
      "query_quota_usage",
      "Query Lilia quota usage summaries through the Lilia internal quota plugin.",
      queryQuotaUsageInputSchema,
      createQuotaUsageHandler(requestQuotaUsage),
      { alwaysLoad: true },
    ),
  ];
  if (conversationContextEnabled(conversationContext)) {
    tools.push(createTool(
      "query_conversation_context",
      buildConversationContextToolDescription(),
      queryConversationContextInputSchema,
      createConversationContextHandler(conversationContext),
      { alwaysLoad: true },
    ));
  }
  if (architectureContextEnabled(conversationContext) && architectureHandler) {
    tools.push(createTool(
      "update_project_architecture",
      "Update Lilia's project-level architecture graph with structured changes.",
      updateProjectArchitectureInputSchema,
      architectureHandler,
      { alwaysLoad: true },
    ));
  }
  return createServer({
    name: "lilia",
    version: "0.1.0",
    tools,
    alwaysLoad: true,
  });
}

function readLiliaWorkflow(cmd) {
  return isRecord(cmd?.workflow) ? cmd.workflow : null;
}

async function runClaudeSessionForkRuntimeCommand(cmd, context, cwd) {
  const command = isRecord(cmd?.runtimeCommand) ? cmd.runtimeCommand : null;
  if (command?.type !== "session_fork") return false;
  const sourceSessionId = typeof cmd.resumeSessionId === "string" ? cmd.resumeSessionId.trim() : "";
  context.protocol.emitTimeline({
    kind: "diagnostic",
    status: "started",
    title: "Claude session fork started",
    summary: "正在分叉 Claude session",
    payload: {
      backend: "claude",
      subkind: "session_fork",
      sourceSessionId: sourceSessionId || null,
    },
    sourceId: `claude:session-fork:start:${sourceSessionId || "missing"}`,
  });
  try {
    if (!sourceSessionId) throw new Error("当前 Claude task 没有可 fork 的 session");
    const forkClaudeSession = context.forkClaudeSession || forkSession;
    const result = await forkClaudeSession(sourceSessionId, { dir: cwd });
    const sessionId = typeof result?.sessionId === "string" ? result.sessionId.trim() : "";
    if (!sessionId) throw new Error("Claude forkSession did not return a session id");
    context.protocol.emitTimeline({
      kind: "diagnostic",
      status: "success",
      title: "Claude session fork completed",
      summary: "已分叉 Claude session",
      payload: {
        backend: "claude",
        subkind: "session_fork",
        sourceSessionId,
        sessionId,
      },
      sourceId: `claude:session-fork:completed:${sourceSessionId}:${sessionId}`,
    });
    context.protocol.emit({ type: "done", sessionId, subtype: "success" });
  } catch (err) {
    context.protocol.emitTimeline({
      kind: "diagnostic",
      status: "error",
      title: "Claude session fork failed",
      summary: err?.message || String(err),
      payload: {
        backend: "claude",
        subkind: "session_fork",
        sourceSessionId: sourceSessionId || null,
        error: err?.message || String(err),
      },
      sourceId: `claude:session-fork:error:${sourceSessionId || "missing"}`,
    });
    throw err;
  }
  return true;
}

function normalizeLiliaReviewTarget(target) {
  if (!isRecord(target)) return null;
  const type = stringOrNull(target.type);
  if (type === "uncommittedChanges") return { type };
  if (type === "baseBranch") {
    const branch = stringOrNull(target.branch)?.trim();
    return branch ? { type, branch } : null;
  }
  if (type === "commit") {
    const sha = stringOrNull(target.sha)?.trim();
    return sha ? { type, sha } : null;
  }
  return null;
}

function liliaReviewTargetText(target) {
  if (target.type === "uncommittedChanges") return "当前工作区未提交改动";
  if (target.type === "baseBranch") return `当前工作区相对分支 ${target.branch} 的差异`;
  return `提交 ${target.sha}`;
}

function readLiliaReviewWorkflow(cmd) {
  const workflow = readLiliaWorkflow(cmd);
  if (workflow?.type !== "lilia_review") return null;
  const target = normalizeLiliaReviewTarget(workflow.target);
  if (!target) throw new Error("Lilia review workflow missing a valid target");
  return {
    target,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
  };
}

function readLiliaFixSuggestionWorkflow(cmd) {
  const workflow = readLiliaWorkflow(cmd);
  if (workflow?.type !== "lilia_fix_suggestion") return null;
  const target = normalizeLiliaReviewTarget(workflow.target);
  if (!target) throw new Error("Lilia fix suggestion workflow missing a valid target");
  return {
    target,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
    mode: workflow.mode === "apply" ? "apply" : "suggest",
  };
}

function readLiliaBatchApplyWorkflow(cmd) {
  const workflow = readLiliaWorkflow(cmd);
  if (workflow?.type !== "lilia_batch_apply") return null;
  const sourceTurnId = stringOrNull(workflow.sourceTurnId)?.trim();
  const sourceKind = stringOrNull(workflow.sourceKind);
  const sourceSummary = stringOrNull(workflow.sourceSummary)?.trim();
  if (!sourceTurnId) throw new Error("Lilia batch apply workflow missing sourceTurnId");
  if (sourceKind !== "review" && sourceKind !== "fix_suggestion") {
    throw new Error("Lilia batch apply workflow missing a valid sourceKind");
  }
  if (!sourceSummary) throw new Error("Lilia batch apply workflow missing sourceSummary");
  return {
    sourceTurnId,
    sourceKind,
    sourceSummary,
    instructions: stringOrNull(workflow.instructions)?.trim() || "",
  };
}

function buildClaudeWorkflowPrompt(cmd, providerSettings = null) {
  const review = readLiliaReviewWorkflow(cmd);
  if (review) {
    return [
      "Lilia code review workflow.",
      `Review target: ${liliaReviewTargetText(review.target)}.`,
      "Focus on bugs, regressions, risky behavior, and missing tests. Put findings first with file and line references where possible.",
      review.instructions ? `User instructions: ${review.instructions}` : null,
      cmd.prompt?.trim() ? `Additional user message: ${cmd.prompt.trim()}` : null,
    ].filter(Boolean).join("\n");
  }
  const fix = readLiliaFixSuggestionWorkflow(cmd);
  if (fix) {
    return [
      "Lilia fix suggestion workflow.",
      `Target: ${liliaReviewTargetText(fix.target)}.`,
      fix.mode === "apply"
        ? "Apply the smallest correct fix for the target. Keep changes scoped."
        : "Suggest a concrete fix plan without editing files unless explicitly necessary.",
      fix.instructions ? `User instructions: ${fix.instructions}` : null,
      cmd.prompt?.trim() ? `Additional user message: ${cmd.prompt.trim()}` : null,
    ].filter(Boolean).join("\n");
  }
  const batch = readLiliaBatchApplyWorkflow(cmd);
  if (batch) {
    return [
      "Lilia batch apply workflow.",
      `Source kind: ${batch.sourceKind}.`,
      `Source turn: ${batch.sourceTurnId}.`,
      "Apply the following reviewed suggestion with the smallest correct code changes.",
      "",
      batch.sourceSummary,
      batch.instructions ? `\nUser instructions: ${batch.instructions}` : null,
      cmd.prompt?.trim() ? `\nAdditional user message: ${cmd.prompt.trim()}` : null,
    ].filter(Boolean).join("\n");
  }
  if (providerSettings) {
    const optionKeys = Object.keys(providerSettings.options);
    const supportedKeys = Object.keys(providerSettings.supported);
    return [
      "Lilia Claude runtime settings command.",
      `Action: ${providerSettings.action}.`,
      "Use the supplied Claude SDK options for this turn and report the effective setting summary briefly.",
      optionKeys.length > 0 ? `Claude option keys: ${optionKeys.join(", ")}.` : null,
      supportedKeys.length > 0 ? `Common setting keys: ${supportedKeys.join(", ")}.` : null,
      cmd.prompt?.trim() ? `Additional user message: ${cmd.prompt.trim()}` : null,
    ].filter(Boolean).join("\n");
  }
  return null;
}

const LILIA_GOAL_ACTIONS = new Set(["set", "refresh", "clear"]);
const LILIA_GOAL_STATUSES = new Set([
  "active",
  "paused",
  "blocked",
  "usageLimited",
  "budgetLimited",
  "complete",
]);
const LILIA_MEMORY_MODES = new Set(["enabled", "disabled"]);
const RUNTIME_SETTINGS_ACTIONS = new Set(["diagnose", "update"]);
const CLAUDE_PROVIDER_SETTING_KEYS = new Set([
  "allowedTools",
  "disallowedTools",
  "additionalDirectories",
  "settingSources",
  "permissionPromptToolName",
  "resumeSessionAt",
  "sessionId",
  "managedSettings",
  "sandbox",
  "outputFormat",
  "sessionStore",
  "maxTurns",
  "maxBudgetUsd",
  "abortAfterMs",
  "includeHookEvents",
  "forwardSubagentText",
  "agentProgressSummaries",
  "continue",
  "tools",
  "settings",
]);
const CLAUDE_QUERY_LILIA_WORKFLOWS = new Set([
  "lilia_review",
  "lilia_fix_suggestion",
  "lilia_batch_apply",
  "lilia_compact",
]);

function normalizeLiliaGoalStatus(status) {
  const value = stringOrNull(status);
  return LILIA_GOAL_STATUSES.has(value) ? value : "active";
}

function emitClaudeWorkflowDone(context, sessionId = null) {
  context.protocol.emit({ type: "done", sessionId, subtype: "success" });
}

function emitClaudeLocalWorkflowTimeline(context, workflow, config) {
  context.protocol.emitTimeline({
    kind: "diagnostic",
    status: config.status || "success",
    title: config.title,
    summary: config.summary,
    payload: {
      backend: "claude",
      source: "lilia",
      subkind: config.subkind,
      workflowType: workflow?.type ?? null,
      ...config.payload,
    },
    sourceId: `claude:lilia-${config.subkind || "workflow"}:${Date.now()}`,
  });
  emitClaudeWorkflowDone(context);
}

function claudeConfigDiagnosticsPayload(cmd) {
  const runtimeExtensions = readClaudeRuntimeExtensions(cmd);
  return {
    cwd: stringOrNull(cmd.cwd),
    model: stringOrNull(cmd.model),
    permission: cmd.permission === "full" || cmd.permission === "readonly" ? cmd.permission : "ask",
    planMode: cmd.planMode === true,
    hasResumeSession: Boolean(stringOrNull(cmd.resumeSessionId)),
    runtimeExtensions: {
      mcpServerCount: Object.keys(runtimeExtensions.mcpServers || {}).length,
      mcpServers: Object.keys(runtimeExtensions.mcpServers || {}),
      skillCount: runtimeExtensions.skills.length,
      skills: runtimeExtensions.skills,
      pluginCount: runtimeExtensions.plugins.length,
      plugins: runtimeExtensions.plugins,
      warningCount: runtimeExtensions.warnings.length,
      warnings: runtimeExtensions.warnings,
    },
  };
}

function readStringList(value) {
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

function readStringOption(value) {
  return stringOrNull(value)?.trim() || null;
}

function readPlainObject(value) {
  return isRecord(value) && Object.keys(value).length > 0 ? { ...value } : null;
}

function readClaudeToolsOption(value) {
  const list = readStringList(value);
  if (list.length > 0) return list;
  if (isRecord(value) && value.type === "preset") {
    const preset = readStringOption(value.preset);
    if (preset) return { type: "preset", preset };
  }
  return null;
}

function readClaudeRuntimeSettingsCommand(command, runtimeOptions = {}) {
  if (command?.type !== "runtime_settings") return null;
  const action = stringOrNull(command.action);
  if (!RUNTIME_SETTINGS_ACTIONS.has(action)) {
    throw new Error("Lilia runtime settings command missing a valid action");
  }
  const provider = isRecord(runtimeOptions.provider) ? runtimeOptions.provider : {};
  const common = isRecord(runtimeOptions.common) ? runtimeOptions.common : {};
  const claude = isRecord(provider.claude)
    ? provider.claude
    : {};
  const ignoredProviderKeys = isRecord(provider.codex) ? Object.keys(provider.codex) : [];
  const unsupportedKeys = Object.keys(claude)
    .filter((key) => !CLAUDE_PROVIDER_SETTING_KEYS.has(key));
  const supported = {};
  const options = {};
  const model = stringOrNull(common.model)?.trim();
  const permission = normalizeRuntimePermission(common.permission);
  if (model) supported.model = model;
  if (permission) supported.permission = permission;

  for (const key of ["allowedTools", "disallowedTools", "additionalDirectories"]) {
    const values = readStringList(claude[key]);
    if (values.length > 0) options[key] = values;
  }
  const tools = readClaudeToolsOption(claude.tools);
  if (tools) options.tools = tools;
  const permissionPromptToolName = readStringOption(claude.permissionPromptToolName);
  if (permissionPromptToolName) options.permissionPromptToolName = permissionPromptToolName;
  const settingsPath = readStringOption(claude.settings);
  const settingsObject = readPlainObject(claude.settings);
  if (settingsPath || settingsObject) options.settings = settingsPath || settingsObject;
  const managedSettings = readPlainObject(claude.managedSettings);
  if (managedSettings) options.managedSettings = managedSettings;
  const settingSources = readStringList(claude.settingSources);
  if (settingSources.length > 0) options.settingSources = settingSources;
  for (const key of ["sandbox", "outputFormat", "sessionStore"]) {
    const object = readPlainObject(claude[key]);
    if (object) options[key] = object;
  }
  if (typeof claude.maxTurns === "number" && Number.isFinite(claude.maxTurns)) {
    options.maxTurns = claude.maxTurns;
  }
  if (typeof claude.maxBudgetUsd === "number" && Number.isFinite(claude.maxBudgetUsd)) {
    options.maxBudgetUsd = claude.maxBudgetUsd;
  }
  for (const key of ["includeHookEvents", "forwardSubagentText", "agentProgressSummaries", "continue"]) {
    if (typeof claude[key] === "boolean") options[key] = claude[key];
  }
  for (const key of ["resumeSessionAt", "sessionId"]) {
    const value = readStringOption(claude[key]);
    if (value) options[key] = value;
  }
  if (typeof claude.abortAfterMs === "number" && Number.isFinite(claude.abortAfterMs) && claude.abortAfterMs > 0) {
    options.abortAfterMs = Math.trunc(claude.abortAfterMs);
  }
  if (action === "update" && Object.keys(supported).length === 0 && Object.keys(options).length === 0) {
    throw new Error("Lilia provider settings update requires at least one valid setting");
  }
  return {
    action,
    supported,
    options,
    settingsKeys: [...Object.keys(supported), ...Object.keys(options)],
    ignoredProviderKeys,
    unsupportedKeys,
  };
}

async function runClaudeLocalLiliaWorkflow(cmd, context) {
  const workflow = readLiliaWorkflow(cmd);
  if (!workflow) return false;
  if (CLAUDE_QUERY_LILIA_WORKFLOWS.has(workflow.type)) return false;
  if (workflow.type === "lilia_goal") {
    const action = stringOrNull(workflow.action);
    if (!LILIA_GOAL_ACTIONS.has(action)) {
      throw new Error("Lilia goal workflow missing a valid action");
    }
    const objective = stringOrNull(workflow.objective)?.trim() || "";
    if (action === "set" && !objective) throw new Error("Lilia goal workflow missing objective");
    const cleared = action === "clear";
    const goal = cleared
      ? null
      : {
        threadId: stringOrNull(cmd.resumeSessionId) || "claude-local",
        objective: objective || "Lilia Goal",
        status: normalizeLiliaGoalStatus(workflow.status),
        tokenBudget: typeof workflow.tokenBudget === "number" ? workflow.tokenBudget : null,
        tokensUsed: 0,
        timeUsedSeconds: 0,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
    context.protocol.emitTimeline({
      kind: "goal",
      status: cleared ? "cancelled" : "info",
      title: cleared ? "Lilia Goal cleared" : "Lilia Goal updated",
      summary: cleared ? "已清除 Lilia Goal" : goal.objective,
      payload: {
        backend: "claude",
        subkind: "lilia_goal",
        action,
        cleared,
        goal,
        goalStatus: goal?.status ?? null,
      },
      sourceId: `claude:lilia-goal:${action}:${Date.now()}`,
    });
    emitClaudeWorkflowDone(context, stringOrNull(cmd.resumeSessionId));
    return true;
  }
  if (workflow.type === "lilia_memory_mode") {
    const mode = stringOrNull(workflow.mode);
    if (!LILIA_MEMORY_MODES.has(mode)) {
      throw new Error("Lilia memory mode workflow missing a valid mode");
    }
    emitClaudeLocalWorkflowTimeline(context, workflow, {
      subkind: "memory_mode",
      title: "Claude memory mode recorded",
      summary: `Lilia memory mode set to ${mode} for Claude UI state`,
      payload: {
        mode,
        native: false,
        reason: "Claude SDK has no equivalent Lilia-controllable memory mode.",
      },
    });
    return true;
  }
  if (workflow.type === "lilia_memory_reset") {
    emitClaudeLocalWorkflowTimeline(context, workflow, {
      subkind: "memory_reset",
      title: "Claude memory reset recorded",
      summary: "Lilia memory reset completed for Claude UI state",
      payload: {
        native: false,
        reason: "Claude SDK has no equivalent global memory reset endpoint.",
      },
    });
    return true;
  }
  if (workflow.type === "lilia_background_terminals_clean") {
    emitClaudeLocalWorkflowTimeline(context, workflow, {
      subkind: "background_terminals_clean",
      title: "Claude background terminals clean completed",
      summary: "No Lilia-managed Claude background terminals required cleanup",
      payload: {
        cleanedCount: 0,
        native: false,
      },
    });
    return true;
  }
  if (workflow.type === "lilia_config_diagnostics") {
    emitClaudeLocalWorkflowTimeline(context, workflow, {
      subkind: "config_diagnostics",
      title: "Claude config diagnostics",
      summary: "Lilia+Claude runtime diagnostics collected",
      payload: {
        includeLayers: workflow.includeLayers !== false,
        ...claudeConfigDiagnosticsPayload(cmd),
      },
    });
    return true;
  }
  return false;
}

function readClaudeCompactWorkflow(cmd) {
  return readLiliaWorkflow(cmd)?.type === "lilia_compact";
}

function applyClaudeRuntimeSettingsCommand(cmd) {
  const command = isRecord(cmd?.runtimeCommand) ? cmd.runtimeCommand : null;
  const settings = readClaudeRuntimeSettingsCommand(
    command,
    isRecord(cmd?.runtimeOptions) ? cmd.runtimeOptions : {},
  );
  if (!settings) return null;
  if (settings.action === "update") {
    if (settings.supported.model) cmd.model = settings.supported.model;
    if (settings.supported.permission) cmd.permission = settings.supported.permission;
  }
  return settings;
}

function emitClaudeProviderSettingsTimeline(context, settings, status = "info") {
  const failed = status === "error";
  context.protocol.emitTimeline({
    kind: "diagnostic",
    status,
    title: failed
      ? "Claude provider settings failed"
      : settings.action === "diagnose"
        ? "Claude provider settings diagnostics"
        : "Claude provider settings applied",
    summary: settings.settingsKeys.length
      ? `当前可识别 ${settings.settingsKeys.length} 项 Claude provider settings`
      : "当前没有可应用的 Claude provider settings 更新",
    payload: {
      backend: "claude",
      subkind: "provider_settings",
      action: settings.action,
      settingsKeys: settings.settingsKeys,
      commonKeys: Object.keys(settings.supported),
      optionKeys: Object.keys(settings.options),
      ...(settings.ignoredProviderKeys.length ? { ignoredProviderKeys: settings.ignoredProviderKeys } : {}),
      ...(settings.unsupportedKeys.length ? { unsupportedKeys: settings.unsupportedKeys } : {}),
      native: true,
    },
    sourceId: `claude:provider-settings:${settings.action}:${Date.now()}`,
  });
}

function runClaudeProviderSettingsRuntimeCommand(cmd, context) {
  const command = isRecord(cmd?.runtimeCommand) ? cmd.runtimeCommand : null;
  const settings = readClaudeRuntimeSettingsCommand(
    command,
    isRecord(cmd?.runtimeOptions) ? cmd.runtimeOptions : {},
  );
  if (!settings || settings.action !== "diagnose") return false;
  emitClaudeProviderSettingsTimeline(context, settings, "info");
  emitClaudeWorkflowDone(context, stringOrNull(cmd.resumeSessionId));
  return true;
}

function normalizeClaudeMcpElicitationPayload(request, ctx) {
  if (!isRecord(request)) return null;
  const serverName = stringOrNull(request.serverName) || stringOrNull(request.mcp_server_name);
  const mode = stringOrNull(request.mode) === "url" ? "url" : "form";
  if (!serverName) return null;
  const payload = {
    threadId: stringOrNull(ctx?.sessionId) || stringOrNull(request.sessionId) || stringOrNull(request.session_id) || "claude",
    turnId: stringOrNull(request.turnId) || stringOrNull(request.turn_id),
    serverName,
    mode,
    message: stringOrNull(request.message) || "",
    _meta: request._meta ?? null,
  };
  if (mode === "form") payload.requestedSchema = request.requestedSchema ?? request.requested_schema ?? null;
  if (mode === "url") {
    payload.url = stringOrNull(request.url) || "";
    payload.elicitationId = stringOrNull(request.elicitationId) || stringOrNull(request.elicitation_id) || "";
  }
  return payload;
}

function claudeMcpElicitationResponse(result) {
  const action = result?.action === "accept" || result?.action === "decline"
    ? result.action
    : "cancel";
  const content = action === "accept" && isRecord(result?.content)
    ? result.content
    : undefined;
  return {
    action,
    ...(content ? { content } : {}),
  };
}

async function requestClaudeMcpElicitation(request, ctx) {
  const payload = normalizeClaudeMcpElicitationPayload(request, ctx);
  if (!payload || !ctx?.interactions?.requestMcpElicitation) {
    return { action: "cancel" };
  }
  const result = await ctx.interactions.requestMcpElicitation(payload, { backend: "claude" });
  return claudeMcpElicitationResponse(result);
}

function emitClaudeCompactTimeline(context, status, sourceSessionId, err = null) {
  const failed = status === "error";
  const completed = status === "success";
  context.protocol.emitTimeline({
    kind: "diagnostic",
    status,
    title: failed
      ? "Claude compact failed"
      : completed
        ? "Claude compact completed"
        : "Claude compact started",
    summary: failed
      ? err?.message || String(err)
      : completed
        ? "Claude 原生上下文压缩已完成"
        : "正在通过 Claude 原生 /compact 压缩上下文",
    payload: {
      backend: "claude",
      subkind: "compact",
      sourceSessionId: sourceSessionId || null,
      method: "/compact",
      ...(failed ? { error: err?.message || String(err) } : {}),
    },
    sourceId: `claude:compact:${failed ? "error" : completed ? "completed" : "start"}:${sourceSessionId || "missing"}`,
  });
}

async function runClaudeCompactWorkflow(cmd, context, workingDir) {
  if (!readClaudeCompactWorkflow(cmd)) return false;
  const sourceSessionId = typeof cmd.resumeSessionId === "string" ? cmd.resumeSessionId.trim() : "";
  emitClaudeCompactTimeline(context, "started", sourceSessionId);
  try {
    if (!sourceSessionId) throw new Error("当前 Claude task 没有可 compact 的 session");
    const result = await runClaudeQueryTurn(
      {
        ...cmd,
        prompt: "",
        resumeSessionId: sourceSessionId,
        planMode: false,
      },
      context,
      workingDir,
      {
        promptStream: claudeCompactPromptStream(),
        emitRuntimeWarnings: false,
        skipPromptSuggestions: true,
        suppressDone: true,
      },
    );
    if (result?.compactResult === "failed") {
      throw new Error(result.compactError || "Claude compact failed");
    }
    emitClaudeCompactTimeline(context, "success", sourceSessionId);
    emitClaudeWorkflowDone(context, result?.sessionId || sourceSessionId);
  } catch (err) {
    emitClaudeCompactTimeline(context, "error", sourceSessionId, err);
    throw err;
  }
  return true;
}

async function runClaudeQueryTurn(cmd, context, workingDir, overrides = {}) {
  const providerSettings = applyClaudeRuntimeSettingsCommand(cmd);
  if (providerSettings?.action === "update") {
    emitClaudeProviderSettingsTimeline(context, providerSettings, "success");
  }
  const { prompt, model, resumeSessionId } = cmd;
  const workflowPrompt = buildClaudeWorkflowPrompt(cmd, providerSettings);
  const runtimeExtensions = readClaudeRuntimeExtensions(cmd);
  const permission = cmd.permission === "full" || cmd.permission === "readonly"
    ? cmd.permission
    : "ask";
  const planMode = cmd.planMode === true;
  const permOpts = mapClaudeInitialPermission(permission, planMode);
  let lastSessionId = null;
  const providerOptions = { ...(providerSettings?.options || {}) };
  const abortAfterMs = providerOptions.abortAfterMs;
  delete providerOptions.abortAfterMs;
  let abortTimer = null;
  if (typeof abortAfterMs === "number" && abortAfterMs > 0) {
    providerOptions.abortController = new AbortController();
    abortTimer = setTimeout(() => {
      providerOptions.abortController.abort();
    }, abortAfterMs);
  }
  const ctx = {
    protocol: context.protocol,
    interactions: context.interactions,
    emitToolConsentTimeline: context.emitToolConsentTimeline,
    emitClaudePlanTimeline,
    claudeStream: createClaudeStreamState(),
    executionPermission: permission,
    query: null,
    latestAssistantText: "",
    deniedTools: new Map(),
    textFragments: new Map(),
    textFragmentsEmittedCount: 0,
    reasoningBlocks: new Map(),
    sawAssistantTextBlock: false,
    resultSeen: false,
    compactResult: null,
    compactError: null,
    activeTools: new Map(),
    commandEdits: new Map(),
    sessionId: stringOrNull(resumeSessionId),
  };
  const liliaAskUserServer = createLiliaAskUserServer({
    requestAskUser: context.interactions.requestAskUser,
    requestQuotaUsage: context.interactions.requestQuotaUsage,
    conversationContext: cmd.conversationContext,
    architectureHandler: createArchitectureChangeHandler({ cmd, ctx, backend: "claude" }),
    createServer: context.createSdkMcpServer || createSdkMcpServer,
    createTool: context.createClaudeTool || tool,
  });
  const options = {
    cwd: workingDir,
    model: model || undefined,
    resume: resumeSessionId || undefined,
    includePartialMessages: true,
    promptSuggestions: overrides.skipPromptSuggestions === true ? false : true,
    canUseTool: createClaudeCanUseTool(ctx),
    hooks: createClaudeHooks(ctx),
    onElicitation: (request) => requestClaudeMcpElicitation(request, ctx),
    systemPrompt: buildClaudeSystemPrompt(context.platform || process.platform),
    mcpServers: {
      lilia: liliaAskUserServer,
      ...runtimeExtensions.mcpServers,
    },
    toolAliases: {
      AskUserQuestion: "mcp__lilia__ask_user_question",
      QueryConversationContext: "mcp__lilia__query_conversation_context",
      UpdateProjectArchitecture: "mcp__lilia__update_project_architecture",
    },
    toolConfig: {
      askUserQuestion: { previewFormat: "markdown" },
      queryConversationContext: { previewFormat: "markdown" },
      updateProjectArchitecture: { previewFormat: "markdown" },
    },
    ...(runtimeExtensions.skills.length > 0 ? { skills: runtimeExtensions.skills } : {}),
    ...(runtimeExtensions.plugins.length > 0 ? { plugins: runtimeExtensions.plugins } : {}),
    ...providerOptions,
    ...permOpts,
  };
  if (overrides.emitRuntimeWarnings !== false) {
    emitRuntimeExtensionWarnings(context.protocol, "claude", runtimeExtensions.warnings);
  }

  const createClaudeQuery = context.createClaudeQuery || query;
  const claudeQuery = createClaudeQuery({
    prompt: overrides.promptStream ?? singleClaudePromptStream(workflowPrompt || prompt),
    options,
  });
  ctx.query = claudeQuery;
  context.interactions?.handleSettingsUpdate?.((update) => {
    applyClaudeRuntimePermission(ctx, update?.permission);
  });
  try {
    for await (const msg of claudeQuery) {
      if (msg.session_id) {
        lastSessionId = msg.session_id;
        ctx.sessionId = msg.session_id;
      }

      try {
        mapClaudeSystemTimeline(msg, ctx);
        switch (msg.type) {
          case "stream_event": {
            handleClaudeStreamEvent(msg, ctx);
            break;
          }
          case "assistant": {
            const content = msg?.message?.content;
            if (Array.isArray(content)) {
              for (const b of content) {
                if (b && b.type === "text") {
                  ctx.sawAssistantTextBlock = true;
                  if (typeof b.text === "string" && b.text.trim()) {
                    ctx.latestAssistantText = b.text;
                  }
                }
                if (b && b.type === "tool_use") {
                  context.protocol.emit({ type: "tool_use", name: b.name, input: b.input });
                  emitClaudeToolTimeline(b, msg, ctx);
                }
              }
            }
            break;
          }
          case "result": {
            ctx.resultSeen = true;
            const errorSummary = msg.is_error
              ? (Array.isArray(msg.errors) ? msg.errors.join("\n") : msg.subtype) || ""
              : "";
            const status = msg.is_error ? "error" : "success";
            const fragmentsEmitted = finalizeClaudeTextFragments(ctx, status);
            sweepActiveClaudeTools(ctx, status, msg?.session_id);
            finalizeClaudeReasoningTimeline(ctx, msg?.session_id || lastSessionId);
            emitClaudeTextResultFallback(ctx, msg, status, fragmentsEmitted, lastSessionId);
            context.protocol.emitTimeline({
              kind: "turn",
              status,
              title: msg.is_error ? "Claude turn failed" : "Claude turn completed",
              summary: errorSummary,
              payload: {
                backend: "claude",
                subtype: msg.subtype,
                stopReason: msg.stop_reason,
                terminalReason: msg.terminal_reason,
                totalCostUsd: msg.total_cost_usd,
                usage: msg.usage,
                modelUsage: msg.modelUsage,
                permissionDenials: msg.permission_denials,
                errors: msg.errors,
                sessionId: msg.session_id || lastSessionId,
              },
              sourceId: msg.uuid,
            });
            if (overrides.suppressDone !== true) {
              context.protocol.emit({
                type: "done",
                sessionId: msg.session_id || lastSessionId,
                subtype: msg.subtype,
              });
            }
            break;
          }
          case "prompt_suggestion": {
            if (typeof msg.suggestion === "string" && msg.suggestion.trim()) {
              context.protocol.emit({
                type: "prompt_suggestion",
                suggestion: msg.suggestion,
                uuid: msg.uuid,
              });
            }
            break;
          }
          case "user":
          case "user_replay": {
            const content = msg?.message?.content;
            if (Array.isArray(content)) {
              for (const b of content) {
                if (b && b.type === "tool_result") {
                  emitClaudeToolResultTimeline(b, msg, ctx);
                }
              }
            }
            break;
          }
          case "system":
          default:
            break;
        }
      } catch (err) {
        context.protocol.emitTimeline({
          kind: "error",
          status: "error",
          title: "Claude event mapping",
          summary: err?.message || String(err),
          payload: {
            backend: "claude",
            rawType: msg?.type,
          },
          sourceId: msg?.uuid,
        });
      }
    }
  } finally {
    if (abortTimer) clearTimeout(abortTimer);
  }
  if (lastSessionId && !ctx.resultSeen) {
    const fragmentsEmitted = finalizeClaudeTextFragments(ctx, "success");
    sweepActiveClaudeTools(ctx, "success", lastSessionId);
    finalizeClaudeReasoningTimeline(ctx, lastSessionId);
    emitClaudeTextResultFallback(ctx, null, "success", fragmentsEmitted, lastSessionId);
    context.protocol.emitTimeline({
      kind: "turn",
      status: "success",
      title: "Claude turn completed",
      summary: "",
      payload: {
        backend: "claude",
        subtype: "success",
        sessionId: lastSessionId,
      },
      sourceId: `${lastSessionId}:turn:done`,
    });
    if (overrides.suppressDone !== true) {
      context.protocol.emit({ type: "done", sessionId: lastSessionId, subtype: "success" });
    }
  }
  return {
    sessionId: lastSessionId,
    compactResult: ctx.compactResult,
    compactError: ctx.compactError,
  };
}

export async function runClaude(cmd, context) {
  handleExperimentalProviderOptions(cmd, context, "claude");
  const { cwd } = cmd;
  const workingDir = cwd || (context.cwd ? context.cwd() : process.cwd());
  if (await runClaudeSessionForkRuntimeCommand(cmd, context, workingDir)) return;
  if (await runClaudeCompactWorkflow(cmd, context, workingDir)) return;
  if (await runClaudeSessionManagementRuntimeCommand(cmd, context, workingDir)) return;
  if (await runClaudeProviderSettingsRuntimeCommand(cmd, context)) return;
  if (await runClaudeLocalLiliaWorkflow(cmd, context)) return;
  await runClaudeQueryTurn(cmd, context, workingDir);
}

export function handleClaudeStreamEvent(msg, ctx) {
  dispatchClaudeStreamEvent({
    event: msg?.event,
    state: ctx.claudeStream,
    onTextStart: ({ blockKey }) => {
      if (blockKey === null || blockKey === undefined) return;
      getOrCreateClaudeTextFragment(ctx, blockKey, msg?.session_id);
      emitAssistantTextFragmentTimeline(ctx.protocol, "", "running", msg?.session_id, blockKey);
    },
    onTextDelta: ({ blockKey, text }) => {
      if (blockKey === null || blockKey === undefined) return;
      const fragment = getOrCreateClaudeTextFragment(ctx, blockKey, msg?.session_id);
      fragment.accumulatedText += text;
      ctx.latestAssistantText = fragment.accumulatedText;
      fragment.sessionId = msg?.session_id || fragment.sessionId;
      fragment.pacer.push(text);
    },
    onTextClose: ({ blockKey }) => {
      if (blockKey === null || blockKey === undefined) return;
      closeClaudeTextFragment(ctx, blockKey, msg?.session_id);
    },
    onReasoning: ({ blockKey, text }) => {
      if (blockKey === null || blockKey === undefined) return;
      const entry = getOrCreateClaudeReasoningBlock(ctx, blockKey, msg?.session_id);
      entry.sessionId = msg?.session_id || entry.sessionId;
      entry.lastText = text;
      entry.pacer.push(text);
    },
    onReasoningClose: ({ blockKey }) => {
      if (blockKey === null || blockKey === undefined) return;
      closeClaudeReasoningBlock(ctx, blockKey, msg?.session_id);
    },
  });
}
