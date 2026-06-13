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

export function createLiliaAskUserServer({
  createServer = createSdkMcpServer,
  createTool = tool,
  requestAskUser,
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

async function runClaudeSessionForkWorkflow(cmd, context, cwd) {
  if (readLiliaWorkflow(cmd)?.type !== "lilia_session_fork") return false;
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

function buildClaudeWorkflowPrompt(cmd) {
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
const CLAUDE_QUERY_LILIA_WORKFLOWS = new Set([
  "lilia_review",
  "lilia_fix_suggestion",
  "lilia_batch_apply",
  "lilia_session_fork",
]);
const CLAUDE_LOCAL_LILIA_DIAGNOSTICS = new Map([
  ["lilia_memory_mode", "Claude 当前没有可由 Lilia 控制的 thread memory mode。"],
  ["lilia_memory_reset", "Claude 当前没有可由 Lilia 重置的全局 memory 接口。"],
  ["lilia_background_terminals_clean", "Claude 当前没有可由 Lilia 清理的后台终端接口。"],
  ["lilia_config_diagnostics", "Claude 当前没有等价的 runtime config diagnostics 接口。"],
  ["lilia_compact", "Claude 当前没有可由 Lilia 手动触发的上下文压缩接口。"],
]);

function normalizeLiliaGoalStatus(status) {
  const value = stringOrNull(status);
  return LILIA_GOAL_STATUSES.has(value) ? value : "active";
}

function emitClaudeWorkflowDone(context, sessionId = null) {
  context.protocol.emit({ type: "done", sessionId, subtype: "success" });
}

function emitClaudeUnsupportedWorkflow(context, workflow, reason) {
  context.protocol.emitTimeline({
    kind: "diagnostic",
    status: "info",
    title: "Lilia workflow handled locally",
    summary: reason,
    payload: {
      backend: "claude",
      subkind: "lilia_workflow",
      workflowType: workflow?.type ?? null,
      reason,
    },
    sourceId: `claude:lilia-workflow:${workflow?.type ?? "unknown"}:${Date.now()}`,
  });
  emitClaudeWorkflowDone(context);
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
  const diagnostic = CLAUDE_LOCAL_LILIA_DIAGNOSTICS.get(workflow.type);
  if (diagnostic) {
    const mode = stringOrNull(workflow.mode);
    if (workflow.type === "lilia_memory_mode" && !LILIA_MEMORY_MODES.has(mode)) {
      throw new Error("Lilia memory mode workflow missing a valid mode");
    }
    emitClaudeUnsupportedWorkflow(context, workflow, diagnostic);
    return true;
  }
  return false;
}

export async function runClaude(cmd, context) {
  const { cwd, prompt, model, resumeSessionId } = cmd;
  const workingDir = cwd || (context.cwd ? context.cwd() : process.cwd());
  if (await runClaudeSessionForkWorkflow(cmd, context, workingDir)) return;
  if (await runClaudeLocalLiliaWorkflow(cmd, context)) return;
  const workflowPrompt = buildClaudeWorkflowPrompt(cmd);
  const runtimeExtensions = readClaudeRuntimeExtensions(cmd);
  const permission = cmd.permission === "full" || cmd.permission === "readonly"
    ? cmd.permission
    : "ask";
  const planMode = cmd.planMode === true;
  const permOpts = mapClaudeInitialPermission(permission, planMode);
  let lastSessionId = null;
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
    activeTools: new Map(),
    commandEdits: new Map(),
  };
  const liliaAskUserServer = createLiliaAskUserServer({
    requestAskUser: context.interactions.requestAskUser,
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
    promptSuggestions: true,
    canUseTool: createClaudeCanUseTool(ctx),
    hooks: createClaudeHooks(ctx),
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
    ...permOpts,
  };
  emitRuntimeExtensionWarnings(context.protocol, "claude", runtimeExtensions.warnings);

  const createClaudeQuery = context.createClaudeQuery || query;
  const claudeQuery = createClaudeQuery({ prompt: singleClaudePromptStream(workflowPrompt || prompt), options });
  ctx.query = claudeQuery;
  context.interactions?.handleSettingsUpdate?.((update) => {
    applyClaudeRuntimePermission(ctx, update?.permission);
  });
  for await (const msg of claudeQuery) {
    if (msg.session_id) lastSessionId = msg.session_id;

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
          context.protocol.emit({
            type: "done",
            sessionId: msg.session_id || lastSessionId,
            subtype: msg.subtype,
          });
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
    context.protocol.emit({ type: "done", sessionId: lastSessionId, subtype: "success" });
  }
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
