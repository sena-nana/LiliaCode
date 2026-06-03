import { buildPromptWithAttachments } from "./prompt.mjs";
import { createProtocolEmitter } from "./protocol.mjs";
import { createInteractionBroker, emitAskUserTimeline } from "./interactions.mjs";
import { emitToolConsentTimeline } from "./toolConsentTimeline.mjs";
import { runClaude } from "./claude/runClaude.mjs";
import { runCodex } from "./codex/runCodex.mjs";
import { runDryRun } from "./dryRun.mjs";

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
  if (typeof cmd?.prompt !== "string") {
    context.protocol.emit({ type: "error", message: "missing prompt" });
    return { ok: false, exitCode: 1 };
  }
  const nextCmd = {
    ...cmd,
    prompt: buildPromptWithAttachments(cmd.prompt, cmd.attachments),
  };
  if (nextCmd.prompt.trim().length === 0) {
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
