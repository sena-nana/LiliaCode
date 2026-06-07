import { createSdkMcpServer, query, tool } from "@anthropic-ai/claude-agent-sdk";
import { createClaudeStreamState, dispatchClaudeStreamEvent } from "../claudeStream.mjs";
import {
  askUserQuestionInputSchema,
  createClaudeAskUserHandler,
} from "../askUser.mjs";
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

export function createLiliaAskUserServer({ createServer = createSdkMcpServer, createTool = tool, requestAskUser }) {
  return createServer({
    name: "lilia",
    version: "0.1.0",
    tools: [
      createTool(
        "ask_user_question",
        "Ask the human user one or more multiple-choice questions through Lilia.",
        askUserQuestionInputSchema,
        createClaudeAskUserHandler(requestAskUser),
        { alwaysLoad: true },
      ),
    ],
    alwaysLoad: true,
  });
}

export async function runClaude(cmd, context) {
  const { cwd, prompt, model, resumeSessionId } = cmd;
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
    createServer: context.createSdkMcpServer || createSdkMcpServer,
    createTool: context.createClaudeTool || tool,
  });
  const options = {
    cwd: cwd || (context.cwd ? context.cwd() : process.cwd()),
    model: model || undefined,
    resume: resumeSessionId || undefined,
    includePartialMessages: true,
    canUseTool: createClaudeCanUseTool(ctx),
    hooks: createClaudeHooks(ctx),
    systemPrompt: buildClaudeSystemPrompt(context.platform || process.platform),
    mcpServers: {
      lilia: liliaAskUserServer,
      ...runtimeExtensions.mcpServers,
    },
    toolAliases: {
      AskUserQuestion: "mcp__lilia__ask_user_question",
    },
    toolConfig: {
      askUserQuestion: { previewFormat: "markdown" },
    },
    ...(runtimeExtensions.skills.length > 0 ? { skills: runtimeExtensions.skills } : {}),
    ...(runtimeExtensions.plugins.length > 0 ? { plugins: runtimeExtensions.plugins } : {}),
    ...permOpts,
  };
  emitRuntimeExtensionWarnings(context.protocol, "claude", runtimeExtensions.warnings);

  const createClaudeQuery = context.createClaudeQuery || query;
  const claudeQuery = createClaudeQuery({ prompt: singleClaudePromptStream(prompt), options });
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
