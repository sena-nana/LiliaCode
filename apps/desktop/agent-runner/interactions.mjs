import { normalizeAskUserResult } from "./askUser.mjs";
import { oneLineSummary, stringOrNull } from "./utils.mjs";

export function createInteractionBroker({
  protocol,
  emitToolConsentTimeline,
  emitAskUserTimeline,
} = {}) {
  const consentPending = new Map();
  let consentSeq = 1;
  const askUserPending = new Map();
  let askUserSeq = 1;

  function requestUserConsent(payload) {
    const id = `consent-${consentSeq++}`;
    emitToolConsentTimeline(id, payload, "requires_action");
    protocol.emit({ type: "consent_request", id, ...payload });
    return new Promise((resolve) => {
      consentPending.set(id, (response) => resolve({ id, ...response }));
    });
  }

  function requestAskUser(spec, options = {}) {
    const id = `ask-${askUserSeq++}`;
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
      protocol.emit({ type: "ask_user_request", id, spec });
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
      resolve({
        decision: msg.decision === "allow" ? "allow" : "deny",
        message: stringOrNull(msg.message) || "",
        updatedInput: msg.updatedInput && typeof msg.updatedInput === "object" && !Array.isArray(msg.updatedInput)
          ? msg.updatedInput
          : null,
      });
      return;
    }
    if (msg.type === "ask_user_response") {
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
