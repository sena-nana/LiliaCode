import { buildPromptWithAttachments } from "./prompt.mjs";
import { createProtocolEmitter } from "./protocol.mjs";
import { createInteractionBroker, emitAskUserTimeline } from "./interactions.mjs";
import { emitToolConsentTimeline } from "./toolConsentTimeline.mjs";
import { runClaude } from "./claude/runClaude.mjs";
import { runCodex } from "./codex/runCodex.mjs";
import { runDryRun } from "./dryRun.mjs";
import {
  liliaRuntimeCommandMetadata,
  liliaWorkflowMetadata,
} from "@lilia/contracts/liliaAgentProtocol.mjs";

function isRecord(value) {
  return value && typeof value === "object" && !Array.isArray(value);
}

export function normalizeRunnerCommand(cmd) {
  const turn = isRecord(cmd?.turn) ? cmd.turn : {};
  const runtimeOptions = isRecord(cmd?.runtimeOptions) ? cmd.runtimeOptions : {};
  return {
    ...cmd,
    turn,
    workflow: isRecord(cmd?.workflow) ? cmd.workflow : null,
    runtimeCommand: isRecord(cmd?.runtimeCommand) ? cmd.runtimeCommand : null,
    runtimeOptions,
  };
}

export function createRunnerContext(deps = {}) {
  const protocol = deps.protocol || createProtocolEmitter({ write: deps.write });
  const emitConsentTimeline = (id, payload, status, decisionMessage = "") =>
    emitToolConsentTimeline(protocol, id, payload, status, decisionMessage);
  const emitAskTimeline = (id, spec, status, result = null, backend = "claude") =>
    emitAskUserTimeline(protocol, id, spec, status, result, backend);
  const interactions = deps.interactions || createInteractionBroker({
    protocol,
    emitToolConsentTimeline: emitConsentTimeline,
    emitAskUserTimeline: emitAskTimeline,
  });
  return {
    ...deps,
    protocol,
    interactions,
    emitToolConsentTimeline: emitConsentTimeline,
  };
}

export async function runAgentTurn(cmd, deps = {}) {
  const context = createRunnerContext(deps);
  cmd = normalizeRunnerCommand(cmd);
  const workflowType = typeof cmd?.workflow?.type === "string" ? cmd.workflow.type : "";
  const runtimeCommandType = typeof cmd?.runtimeCommand?.type === "string" ? cmd.runtimeCommand.type : "";
  const workflowMetadata = workflowType ? liliaWorkflowMetadata(workflowType) : null;
  const runtimeCommandMetadata = runtimeCommandType ? liliaRuntimeCommandMetadata(runtimeCommandType) : null;
  if (workflowType && !workflowMetadata) {
    context.protocol.emit({ type: "error", message: `unknown workflow: ${workflowType}` });
    return { ok: false, exitCode: 1 };
  }
  if (runtimeCommandType && !runtimeCommandMetadata) {
    context.protocol.emit({ type: "error", message: `unknown runtimeCommand: ${runtimeCommandType}` });
    return { ok: false, exitCode: 1 };
  }
  const turn = isRecord(cmd.turn) ? cmd.turn : null;
  if (typeof turn?.prompt !== "string") {
    context.protocol.emit({ type: "error", message: "missing prompt" });
    return { ok: false, exitCode: 1 };
  }
  const turnAttachments = Array.isArray(turn.attachments) ? turn.attachments : undefined;
  const nextCmd = {
    ...cmd,
    prompt: buildPromptWithAttachments(turn.prompt, turnAttachments),
    cwd: typeof turn.cwd === "string" ? turn.cwd : undefined,
    attachments: turnAttachments,
    model: typeof turn.model === "string" ? turn.model : undefined,
    resumeSessionId: typeof turn.resumeSessionId === "string" ? turn.resumeSessionId : undefined,
    planMode: typeof turn.planMode === "boolean" ? turn.planMode : undefined,
    goalMode: typeof turn.goalMode === "boolean" ? turn.goalMode : undefined,
    autoSessionFork: cmd.runtimeOptions?.common?.modelSelection?.sessionFork === true,
    permission: typeof turn.permission === "string" ? turn.permission : undefined,
  };
  const allowsEmptyWorkflowPrompt = workflowMetadata?.requiresPrompt === false;
  const allowsEmptyRuntimeCommandPrompt = runtimeCommandMetadata?.requiresPrompt === false;
  if (
    nextCmd.prompt.trim().length === 0 &&
    !allowsEmptyWorkflowPrompt &&
    !allowsEmptyRuntimeCommandPrompt
  ) {
    context.protocol.emit({ type: "error", message: "missing prompt" });
    return { ok: false, exitCode: 1 };
  }

  try {
    if ((context.env || process.env).LILIA_AGENT_DRY_RUN === "1") {
      await (context.runDryRun || runDryRun)(nextCmd, context);
      return { ok: true, exitCode: 0 };
    }

    const backend = nextCmd.backend === "codex" ? "codex" : "claude";
    if (backend === "codex") {
      await (context.runCodex || runCodex)(nextCmd, context);
    } else {
      await (context.runClaude || runClaude)(nextCmd, context);
    }
    return { ok: true, exitCode: 0 };
  } catch (err) {
    context.protocol.emit({ type: "error", message: err?.message || String(err) });
    return { ok: false, exitCode: 1 };
  }
}
