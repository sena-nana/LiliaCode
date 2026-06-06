import { oneLineSummary, stringOrNull } from "../utils.mjs";
import { stringifyCodexCommand } from "../toolConsentTimeline.mjs";

const EDIT_EXEC_OUTPUT_LIMIT = 6000;
const EDIT_EXEC_WAIT_MS = 1000 * 60 * 30;

export function mapCodexPermission(p) {
  switch (p) {
    case "full":
      return { sandboxMode: "danger-full-access" };
    case "readonly":
      return { sandboxMode: "read-only" };
    case "ask":
    default:
      return { sandboxMode: "workspace-write" };
  }
}

export function mapCodexSandboxMode(permission) {
  return mapCodexPermission(permission).sandboxMode;
}

export function mapCodexApprovalPolicy(permission) {
  if (permission === "full") return "never";
  if (permission === "readonly") return "never";
  return "on-request";
}

function isRecord(value) {
  return value && typeof value === "object" && !Array.isArray(value);
}

function codexApprovalDecisionFromConsent(response, availableDecisions) {
  const explicit = stringOrNull(response?.codexDecision);
  if (explicit) return explicit;
  if (response?.decision === "allow") return "accept";
  if (Array.isArray(availableDecisions) && !availableDecisions.includes("decline")) {
    return codexCancelDecision(availableDecisions);
  }
  return "decline";
}

function codexCancelDecision(availableDecisions) {
  if (Array.isArray(availableDecisions) && availableDecisions.includes("cancel")) {
    return "cancel";
  }
  return "decline";
}

function readAvailableDecisions(params) {
  return Array.isArray(params?.availableDecisions)
    ? params.availableDecisions.filter((item) => typeof item === "string")
    : ["accept", "decline"];
}

function buildCodexApprovalPayload(toolName, params, requestId, title, description) {
  const input = { ...params };
  const availableDecisions = readAvailableDecisions(params);
  return {
    backend: "codex",
    toolName,
    input,
    toolUseID: requestId,
    title,
    displayName: toolName,
    description,
    additionalPermissions: params.additionalPermissions ?? null,
    availableDecisions,
    proposedExecpolicyAmendment: params.proposedExecpolicyAmendment ?? null,
    proposedNetworkPolicyAmendments: params.proposedNetworkPolicyAmendments ?? null,
    networkApprovalContext: params.networkApprovalContext ?? null,
    cwd: stringOrNull(params.cwd),
    reason: stringOrNull(params.reason),
    commandActions: params.commandActions ?? null,
  };
}

function readCodexCommandFromInput(input) {
  return stringifyCodexCommand(
    input?.parsedCmd ||
      input?.command ||
      input?.cmd ||
      input?.commandActions,
  );
}

function readAllowedCodexCommandEdit(originalInput, updatedInput) {
  if (!isRecord(originalInput) || !isRecord(updatedInput)) return null;
  const originalCommand = readCodexCommandFromInput(originalInput);
  const modifiedCommand = stringifyCodexCommand(
    updatedInput.parsedCmd ||
      updatedInput.command ||
      updatedInput.cmd ||
      updatedInput.commandActions,
  );
  if (!modifiedCommand.trim() || modifiedCommand === originalCommand) return null;
  return {
    input: { ...originalInput, ...updatedInput, command: modifiedCommand },
    originalCommand,
    modifiedCommand,
  };
}

function trimOutput(value, limit = EDIT_EXEC_OUTPUT_LIMIT) {
  const text = typeof value === "string" ? value : "";
  if (text.length <= limit) return text;
  return `${text.slice(0, limit)}\n...输出已截断，完整输出过长`;
}

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function normalizeExecResult(result, fallback = {}) {
  const row = isRecord(result) ? result : {};
  const nested = isRecord(row.result) ? row.result : {};
  const output = stringOrNull(row.output) ||
    stringOrNull(row.text) ||
    stringOrNull(row.content) ||
    stringOrNull(nested.output) ||
    stringOrNull(fallback.output) ||
    "";
  const stdout = stringOrNull(row.stdout) ||
    stringOrNull(nested.stdout) ||
    stringOrNull(fallback.stdout) ||
    output;
  const stderr = stringOrNull(row.stderr) ||
    stringOrNull(nested.stderr) ||
    stringOrNull(fallback.stderr) ||
    "";
  const exitCode = numberOrNull(row.exitCode) ??
    numberOrNull(row.exit_code) ??
    numberOrNull(row.code) ??
    numberOrNull(nested.exitCode) ??
    numberOrNull(nested.exit_code) ??
    numberOrNull(fallback.exitCode) ??
    0;
  return {
    exitCode,
    stdout: trimOutput(stdout),
    stderr: trimOutput(stderr),
    output: trimOutput(output || [stdout, stderr].filter(Boolean).join("\n")),
  };
}

function processIdFromSpawnResult(result) {
  if (!isRecord(result)) return null;
  return stringOrNull(result.processId) ||
    stringOrNull(result.processID) ||
    stringOrNull(result.pid) ||
    stringOrNull(result.id);
}

function notificationMethod(msg) {
  return stringOrNull(msg?.method) || stringOrNull(msg?.type) || "";
}

function notificationParams(msg) {
  return isRecord(msg?.params) ? msg.params : isRecord(msg) ? msg : {};
}

function outputDeltaFromNotification(msg) {
  const params = notificationParams(msg);
  return stringOrNull(params.delta) ||
    stringOrNull(params.text) ||
    stringOrNull(params.chunk) ||
    stringOrNull(params.output) ||
    "";
}

function notificationProcessId(msg) {
  const params = notificationParams(msg);
  return stringOrNull(params.processId) ||
    stringOrNull(params.processID) ||
    stringOrNull(params.pid) ||
    stringOrNull(params.id);
}

function notificationMatchesProcess(msg, processId) {
  if (!processId) return true;
  const id = notificationProcessId(msg);
  return !id || id === processId;
}

async function waitForSpawnedProcess(server, processId) {
  const startedAt = Date.now();
  let stdout = "";
  let stderr = "";
  while (Date.now() - startedAt < EDIT_EXEC_WAIT_MS) {
    for (const msg of server.drainNotifications?.() || []) {
      const method = notificationMethod(msg);
      if (!method.startsWith("process/") || !notificationMatchesProcess(msg, processId)) {
        continue;
      }
      if (method === "process/outputDelta") {
        const params = notificationParams(msg);
        const delta = outputDeltaFromNotification(msg);
        if (params.stream === "stderr" || params.fd === 2) stderr += delta;
        else stdout += delta;
        stdout = trimOutput(stdout);
        stderr = trimOutput(stderr);
      }
      if (method === "process/exited") {
        const params = notificationParams(msg);
        return normalizeExecResult({
          exitCode: numberOrNull(params.exitCode) ?? numberOrNull(params.code) ?? 0,
          stdout,
          stderr,
        });
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error("Edited Codex command execution timed out");
}

async function executeEditedCodexCommand(server, edit, cwd) {
  const params = {
    command: edit.modifiedCommand,
    cwd: cwd || undefined,
  };
  try {
    return normalizeExecResult(await server.request("command/exec", params));
  } catch (commandExecError) {
    try {
      const spawned = await server.request("process/spawn", params);
      return await waitForSpawnedProcess(server, processIdFromSpawnResult(spawned));
    } catch (spawnError) {
      const err = new Error(spawnError?.message || commandExecError?.message || "Edited Codex command execution failed");
      err.commandExecError = commandExecError;
      err.spawnError = spawnError;
      throw err;
    }
  }
}

function emitLiliaEditedCommandTimeline(ctx, payload, edit, result, status = "success", message = "") {
  const exitCode = numberOrNull(result?.exitCode);
  const summary = message ||
    (exitCode === 0 ? "Lilia 已执行用户修改后的命令" : `Lilia 执行用户修改后的命令失败，退出码 ${exitCode ?? "unknown"}`);
  ctx.protocol.emitTimeline({
    kind: "command",
    status,
    title: edit.modifiedCommand,
    summary,
    payload: {
      backend: "codex",
      executionOwner: "lilia",
      subkind: "lilia_edit_exec",
      originalCommand: edit.originalCommand,
      modifiedCommand: edit.modifiedCommand,
      command: edit.modifiedCommand,
      cwd: stringOrNull(payload?.cwd),
      exitCode,
      stdout: stringOrNull(result?.stdout),
      stderr: stringOrNull(result?.stderr),
      output: stringOrNull(result?.output),
    },
    sourceId: `${payload.toolUseID || "codex-command"}:lilia-edit-exec`,
  });
}

function commandFence(command) {
  let fence = "```";
  while (command.includes(fence)) fence += "`";
  return fence;
}

function buildEditedCommandAdditionalContext(edit, result) {
  const fence = commandFence(edit.modifiedCommand);
  const output = trimOutput(result?.output || [result?.stdout, result?.stderr].filter(Boolean).join("\n"), 4000);
  return [
    "用户编辑了 Codex 请求审批的命令，Lilia 已执行编辑后的命令；不要再执行原始命令。",
    "",
    "原始命令：",
    `${fence}shell`,
    edit.originalCommand,
    fence,
    "",
    "编辑后由 Lilia 执行的命令：",
    `${fence}shell`,
    edit.modifiedCommand,
    fence,
    "",
    `退出码：${numberOrNull(result?.exitCode) ?? "unknown"}`,
    output ? `输出摘要：\n${output}` : "输出摘要：无输出",
  ].join("\n");
}

async function steerCodexWithEditedCommandResult(server, ctx, edit, result) {
  await server.request("turn/steer", {
    threadId: ctx.threadId,
    turnId: ctx.currentTurnId,
    additionalContext: buildEditedCommandAdditionalContext(edit, result),
  });
}

export async function maybeHandleCodexApprovalRequest(server, msg, ctx) {
  const method = stringOrNull(msg.method) || "";
  const params = msg.params && typeof msg.params === "object" && !Array.isArray(msg.params)
    ? msg.params
    : {};
  const isCommandApproval =
    method === "item/commandExecution/requestApproval";
  const isFileChangeApproval =
    method === "item/fileChange/requestApproval";
  if (!method.endsWith("/requestApproval")) return false;
  const requestId =
    stringOrNull(params.approvalId) ||
    stringOrNull(params.callId) ||
    stringOrNull(params.itemId) ||
    stringOrNull(msg.id) ||
    `codex-${method}`;
  const toolName = method;
  const command = stringifyCodexCommand(
    params.command ||
      params.parsedCmd ||
      params.cmd ||
      params.commandActions,
  );
  const description = isCommandApproval
    ? command
    : oneLineSummary(params.grantRoot || params.reason || method);
  const title = isCommandApproval
    ? "Codex command approval"
    : isFileChangeApproval
      ? "Codex patch approval"
      : "Codex tool approval";
  const payload = buildCodexApprovalPayload(toolName, params, requestId, title, description);
  const response = await ctx.interactions.requestUserConsent(payload);
  const { id, decision, message, updatedInput } = response;
  if (isCommandApproval && decision === "allow" && !response.codexDecision) {
    const edit = readAllowedCodexCommandEdit(payload.input, updatedInput);
    if (edit) {
      try {
        const result = await executeEditedCodexCommand(server, edit, payload.cwd);
        emitLiliaEditedCommandTimeline(ctx, payload, edit, result, result.exitCode === 0 ? "success" : "error");
        server.respond(msg.id, { decision: codexCancelDecision(payload.availableDecisions) });
        try {
          await steerCodexWithEditedCommandResult(server, ctx, edit, result);
        } catch (err) {
          ctx.protocol.emitTimeline({
            kind: "error",
            status: "error",
            title: "Codex edited command context",
            summary: err?.message || String(err),
            payload: {
              backend: "codex",
              executionOwner: "lilia",
              subkind: "lilia_edit_exec_context",
              originalCommand: edit.originalCommand,
              modifiedCommand: edit.modifiedCommand,
            },
            sourceId: `${payload.toolUseID || "codex-command"}:lilia-edit-exec-context`,
          });
        }
        ctx.emitToolConsentTimeline(id, {
          ...payload,
          input: edit.input,
          commandEdited: true,
          originalCommand: edit.originalCommand,
          modifiedCommand: edit.modifiedCommand,
        }, "success", "Lilia 已执行用户修改后的 Codex 命令");
        return true;
      } catch (err) {
        emitLiliaEditedCommandTimeline(ctx, payload, edit, {
          exitCode: null,
          stdout: "",
          stderr: err?.message || String(err),
          output: err?.message || String(err),
        }, "error", "Lilia 无法执行用户修改后的命令");
        server.respond(msg.id, { decision: codexCancelDecision(payload.availableDecisions) });
        ctx.emitToolConsentTimeline(id, payload, "cancelled", "Lilia 无法执行用户修改后的命令，已取消原始 Codex 命令");
        return true;
      }
    }
  }
  const codexDecision = codexApprovalDecisionFromConsent(response, payload.availableDecisions);
  server.respond(msg.id, { decision: codexDecision });
  const accepted = codexDecision === "accept";
  ctx.emitToolConsentTimeline(id, payload, accepted ? "success" : "cancelled", accepted
    ? "用户已同意此次 Codex 操作"
    : message || "用户拒绝了此次 Codex 操作");
  return true;
}
