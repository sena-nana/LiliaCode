import { normalizeAskUserResult } from "./askUser.mjs";
import { oneLineSummary, stringOrNull } from "./utils.mjs";

export function normalizeToolConsentResult(value) {
  const row = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  return {
    decision: row.decision === "allow" ? "allow" : "deny",
    message: stringOrNull(row.message) || "",
    updatedInput: row.updatedInput && typeof row.updatedInput === "object" && !Array.isArray(row.updatedInput)
      ? row.updatedInput
      : null,
  };
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
      consentPending.set(id, (response) => resolve({ id, ...response }));
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
      askUserPending.set(id, (result) => {
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
      });
      emitInteractionRequest(id, kind, spec, backend);
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
    if (msg.type === "consent_response") {
      const resolve = consentPending.get(msg.id);
      if (!resolve) return;
      consentPending.delete(msg.id);
      resolve(normalizeToolConsentResult(msg));
      return;
    }
    if (msg.type === "ask_user_response") {
      const resolve = askUserPending.get(msg.id);
      if (!resolve) return;
      askUserPending.delete(msg.id);
      resolve(normalizeAskUserResult(msg.result));
      return;
    }
    if (msg.type === "interaction_response") {
      const kind = msg.kind === "tool_consent"
        ? "tool_consent"
        : msg.kind === "plan_approval"
          ? "plan_approval"
          : "ask_user";
      if (kind === "tool_consent") {
        const resolve = consentPending.get(msg.id);
        if (!resolve) return;
        consentPending.delete(msg.id);
        resolve(normalizeToolConsentResult(msg.result));
        return;
      }
      const resolve = askUserPending.get(msg.id);
      if (!resolve) return;
      askUserPending.delete(msg.id);
      resolve(normalizeAskUserResult(msg.result));
    }
  }

  return {
    requestUserConsent,
    requestAskUser,
    handleControlLine,
    pendingCounts: () => ({
      consent: consentPending.size,
      askUser: askUserPending.size,
    }),
  };
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
