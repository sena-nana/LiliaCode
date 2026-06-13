import { normalizeAskUserResult } from "./askUser.mjs";
import { emitArchitectureTimeline } from "./architecture.mjs";
import { oneLineSummary, stringOrNull } from "./utils.mjs";

export function normalizeToolConsentResult(value) {
  const row = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  return {
    decision: row.decision === "allow" ? "allow" : "deny",
    message: stringOrNull(row.message) || "",
    updatedInput: row.updatedInput && typeof row.updatedInput === "object" && !Array.isArray(row.updatedInput)
      ? row.updatedInput
      : null,
    codexDecision: stringOrNull(row.codexDecision),
  };
}

function normalizeMcpElicitationResult(value) {
  const row = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  const action = row.action === "accept" || row.action === "decline" ? row.action : "cancel";
  const content = row.content && typeof row.content === "object" && !Array.isArray(row.content)
    ? row.content
    : null;
  return {
    action,
    content,
    _meta: row._meta ?? null,
  };
}

function normalizePermissionApprovalResult(value) {
  const row = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  return {
    permissions: row.permissions && typeof row.permissions === "object" && !Array.isArray(row.permissions)
      ? row.permissions
      : {},
    scope: row.scope || "turn",
    ...(typeof row.strictAutoReview === "boolean"
      ? { strictAutoReview: row.strictAutoReview }
      : {}),
  };
}

const INTERACTION_RESPONSE_KINDS = new Set([
  "tool_consent",
  "plan_approval",
  "ask_user",
  "mcp_elicitation",
  "permission_approval",
  "architecture_change",
]);

function isCodexInteractionKind(kind) {
  return kind === "mcp_elicitation" || kind === "permission_approval";
}

function normalizeCodexInteractionResult(kind, result) {
  return kind === "mcp_elicitation"
    ? normalizeMcpElicitationResult(result)
    : normalizePermissionApprovalResult(result);
}

export function createInteractionBroker({
  protocol,
  emitToolConsentTimeline,
  emitAskUserTimeline,
} = {}) {
  const consentPending = new Map();
  let consentSeq = 1;
  const askUserPending = new Map();
  let askUserSeq = 1;
  const codexPending = new Map();
  let codexSeq = 1;
  const architecturePending = new Map();
  let architectureSeq = 1;
  let settingsUpdateHandler = null;
  let codexIabResultHandler = null;

  function emitInteractionRequest(id, kind, payload, backend = "claude") {
    protocol.emit({
      type: "interaction_request",
      id,
      kind,
      backend,
      payload,
    });
  }

  function requestUserConsent(payload) {
    const id = `consent-${consentSeq++}`;
    emitToolConsentTimeline(id, payload, "requires_action");
    const backend = payload?.backend === "codex" ? "codex" : "claude";
    emitInteractionRequest(id, "tool_consent", payload, backend);
    return new Promise((resolve) => {
      consentPending.set(id, {
        kind: "tool_consent",
        resolve: (response) => resolve({ id, ...response }),
      });
    });
  }

  function requestAskUser(spec, options = {}) {
    const id = `ask-${askUserSeq++}`;
    const kind = spec?.intent === "plan_approval" ? "plan_approval" : "ask_user";
    const emitTimelineEvent =
      options.emitTimelineEvent !== false && spec?.intent !== "plan_approval";
    const backend = options.backend === "codex" ? "codex" : "claude";
    if (emitTimelineEvent) emitAskUserTimeline(id, spec, "requires_action", null, backend);
    return new Promise((resolve) => {
      askUserPending.set(id, {
        kind,
        resolve: (result) => {
          if (emitTimelineEvent) {
            emitAskUserTimeline(
              id,
              spec,
              result?.cancelled === true ? "cancelled" : "success",
              result,
              backend,
            );
          }
          resolve(result);
        },
      });
      emitInteractionRequest(id, kind, spec, backend);
    });
  }

  function emitCodexInteractionTimeline(id, kind, payload, status, result = null) {
    const isMcp = kind === "mcp_elicitation";
    const accepted = result?.action === "accept";
    protocol.emitTimeline({
      kind: isMcp ? "mcp" : "diagnostic",
      status,
      title: isMcp
        ? "Codex MCP elicitation"
        : "Codex permission approval",
      summary: isMcp
        ? oneLineSummary(payload?.message || payload?.serverName || "MCP 请求用户输入")
        : oneLineSummary(payload?.reason || "Codex 请求额外权限"),
      payload: {
        backend: "codex",
        interaction: kind,
        requestId: id,
        ...(isMcp
          ? {
              subkind: "mcp_elicitation",
              serverName: stringOrNull(payload?.serverName),
              mode: stringOrNull(payload?.mode),
              message: stringOrNull(payload?.message),
              url: stringOrNull(payload?.url),
              requestedSchema: payload?.requestedSchema ?? null,
            }
          : {
              subkind: "permission_approval",
              threadId: stringOrNull(payload?.threadId),
              turnId: stringOrNull(payload?.turnId),
              itemId: stringOrNull(payload?.itemId),
              cwd: stringOrNull(payload?.cwd),
              reason: stringOrNull(payload?.reason),
              permissions: payload?.permissions ?? null,
            }),
        ...(result
          ? {
              result,
              accepted: isMcp ? accepted : status === "success",
            }
          : {}),
      },
      sourceId: id,
    });
  }

  function requestCodexInteraction(kind, payload) {
    const id = `codex-${codexSeq++}`;
    emitCodexInteractionTimeline(id, kind, payload, "requires_action");
    emitInteractionRequest(id, kind, payload, "codex");
    return new Promise((resolve) => {
      codexPending.set(id, {
        kind,
        resolve: (result) => {
          emitCodexInteractionTimeline(
            id,
            kind,
            payload,
            codexInteractionCompletedStatus(kind, result),
            result,
          );
          resolve(result);
        },
      });
    });
  }

  function requestArchitectureChange(payload, options = {}) {
    const id = `architecture-${architectureSeq++}`;
    const backend = options.backend === "codex" ? "codex" : "claude";
    const autoApply = options.autoApply === true;
    const requestPayload = {
      ...payload,
      requestId: id,
      status: autoApply ? "applied" : "pending",
      requiresConfirmation: !autoApply,
    };
    emitArchitectureTimeline({ protocol }, id, requestPayload, autoApply ? "info" : "requires_action");
    emitInteractionRequest(id, "architecture_change", requestPayload, backend);
    return new Promise((resolve) => {
      architecturePending.set(id, {
        kind: "architecture_change",
        resolve: (result) => {
          emitArchitectureTimeline(
            { protocol },
            id,
            requestPayload,
            result?.decision === "allow" ? "success" : "cancelled",
            result,
          );
          resolve(result);
        },
      });
    });
  }

  function handleControlLine(line) {
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (!msg || typeof msg !== "object" || Array.isArray(msg)) return;
    if (msg.type === "interaction_response") {
      if (typeof msg.id !== "string") return;
      const kind = msg.kind;
      if (!INTERACTION_RESPONSE_KINDS.has(kind)) return;
      if (kind === "tool_consent") {
        const pending = consentPending.get(msg.id);
        if (!pending || pending.kind !== kind) return;
        consentPending.delete(msg.id);
        pending.resolve(normalizeToolConsentResult(msg.result));
        return;
      }
      if (isCodexInteractionKind(kind)) {
        const pending = codexPending.get(msg.id);
        if (!pending || pending.kind !== kind) return;
        codexPending.delete(msg.id);
        pending.resolve(normalizeCodexInteractionResult(kind, msg.result));
        return;
      }
      if (kind === "architecture_change") {
        const pending = architecturePending.get(msg.id);
        if (!pending || pending.kind !== kind) return;
        architecturePending.delete(msg.id);
        pending.resolve(normalizeArchitectureChangeResult(msg.result));
        return;
      }
      const pending = askUserPending.get(msg.id);
      if (!pending || pending.kind !== kind) return;
      askUserPending.delete(msg.id);
      pending.resolve(normalizeAskUserResult(msg.result));
      return;
    }
    if (msg.type === "settings_update") {
      settingsUpdateHandler?.(msg);
      return;
    }
    if (msg.type === "codex_iab_result") {
      codexIabResultHandler?.(msg.snapshot);
    }
  }

  return {
    requestUserConsent,
    requestAskUser,
    requestCodexInteraction,
    requestArchitectureChange,
    handleControlLine,
    handleSettingsUpdate: (handler) => {
      settingsUpdateHandler = typeof handler === "function" ? handler : null;
    },
    handleCodexIabResult: (handler) => {
      codexIabResultHandler = typeof handler === "function" ? handler : null;
    },
    pendingCounts: () => ({
      consent: consentPending.size,
      askUser: askUserPending.size,
      codex: codexPending.size,
      architecture: architecturePending.size,
    }),
  };
}

function normalizeArchitectureChangeResult(value) {
  const row = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  return {
    decision: row.decision === "allow" ? "allow" : "deny",
    graph: row.graph ?? null,
    event: row.event ?? null,
    message: stringOrNull(row.message),
  };
}

function codexInteractionCompletedStatus(kind, result) {
  if (kind === "mcp_elicitation") {
    return result?.action === "accept" ? "success" : "cancelled";
  }
  if (kind === "permission_approval") {
    return result?.strictAutoReview === true &&
      Object.keys(result?.permissions || {}).length === 0
      ? "cancelled"
      : "success";
  }
  return "success";
}

export function askUserTimelineSummary(spec) {
  const questions = Array.isArray(spec?.questions) ? spec.questions : [];
  const first = questions[0];
  const question = oneLineSummary(first?.question || first?.header || spec?.title || "用户提问");
  if (questions.length <= 1) return question;
  return `${question} 等 ${questions.length} 个问题`;
}

export function emitAskUserTimeline(protocol, id, spec, status, result = null, backend = "claude") {
  const questions = Array.isArray(spec?.questions) ? spec.questions : [];
  protocol.emitTimeline({
    kind: "ask_user",
    status,
    title: stringOrNull(spec?.title) || "AskUserQuestion",
    summary: askUserTimelineSummary(spec),
    payload: {
      backend,
      interaction: "ask_user",
      requestId: id,
      title: stringOrNull(spec?.title),
      source: stringOrNull(spec?.source),
      questions,
      spec,
      ...(result ? { result } : {}),
      cancelled: result?.cancelled === true,
    },
    sourceId: id,
  });
}
