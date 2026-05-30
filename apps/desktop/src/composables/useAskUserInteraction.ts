import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import type {
  AskUserAnswer,
  AskUserOption,
  AskUserQuestion,
  AskUserResult,
} from "@lilia/contracts";
import type { PendingAsk } from "./useAskUser";

export const OTHER_ANSWER_VALUE = "other";

export function useAskUserInteraction(
  activeAsk: ComputedRef<PendingAsk | null>,
  freeformText: Ref<string>,
  resolve: (result: AskUserResult) => void,
) {
  const askIndex = ref(0);
  const askAnswers = ref<Record<string, AskUserAnswer>>({});
  const singleFocus = ref<string | null>(null);
  const activeOptionId = ref<string | null>(null);
  const singlePick = ref<string | null>(null);
  const multiPicks = ref<Set<string>>(new Set());

  const askKey = computed(() => activeAsk.value ? `ask:${activeAsk.value.id}` : "ask:none");
  const askTotal = computed(() => activeAsk.value?.spec.questions.length ?? 0);
  const askQuestion = computed<AskUserQuestion | null>(() =>
    activeAsk.value?.spec.questions[askIndex.value] ?? null,
  );
  const askDismissable = computed(() => activeAsk.value?.spec.dismissable !== false);
  const askIsLast = computed(() => askIndex.value >= askTotal.value - 1);
  const canGoPrev = computed(() => askIndex.value > 0);
  const askTitle = computed(() => {
    const ask = activeAsk.value;
    if (!ask) return "";
    if (ask.spec.title) return ask.spec.title;
    return askTotal.value > 1 ? `Lilia 想确认 ${askTotal.value} 件事` : "Lilia 想确认一下";
  });
  const askIsPlanApproval = computed(() =>
    activeAsk.value?.spec.intent === "plan_approval" &&
    askTotal.value === 1 &&
    askQuestion.value?.mode === "confirm",
  );
  const askOptionsWithId = computed<(AskUserOption & { id: string })[]>(() => {
    const q = askQuestion.value;
    if (!q || !q.options) return [];
    return q.options.map((opt, i) => ({ ...opt, id: opt.id ?? opt.label ?? `opt-${i}` }));
  });
  const askHasPreview = computed(() => askOptionsWithId.value.some((opt) => !!opt.preview));
  const askFocusedOption = computed(() =>
    askOptionsWithId.value.find((opt) => opt.id === singleFocus.value) ?? null,
  );
  const canAskSubmit = computed(() => {
    const q = askQuestion.value;
    if (!q) return false;
    if (q.mode === "confirm") return true;
    const hasFreeform = freeformText.value.trim().length > 0;
    if (q.mode === "single") return hasFreeform || !!singlePick.value;
    const min = q.minSelections ?? 1;
    return multiPicks.value.size + (hasFreeform ? 1 : 0) >= min;
  });

  function focusOption(id: string) {
    singleFocus.value = id;
  }

  function highlightOption(id: string) {
    singleFocus.value = id;
    activeOptionId.value = id;
  }

  function clearOptionHighlight(id: string) {
    if (activeOptionId.value !== id) return;
    activeOptionId.value = null;
    singleFocus.value = singlePick.value ?? null;
  }

  function selectSingleOption(id: string) {
    singleFocus.value = id;
    singlePick.value = id;
  }

  function toggleMulti(id: string) {
    const next = new Set(multiPicks.value);
    if (next.has(id)) next.delete(id);
    else {
      const q = askQuestion.value;
      if (q?.maxSelections && next.size >= q.maxSelections) {
        const first = next.values().next();
        if (!first.done) next.delete(first.value);
      }
      next.add(id);
    }
    multiPicks.value = next;
    singleFocus.value = id;
  }

  function buildAskAnswer(): AskUserAnswer | null {
    const q = askQuestion.value;
    if (!q) return null;
    if (q.mode === "confirm") return { questionId: q.id, value: "yes" };
    if (q.mode === "single") {
      const id = singlePick.value;
      return id ? { questionId: q.id, value: id } : null;
    }
    return { questionId: q.id, value: [...multiPicks.value] };
  }

  function buildFreeformAnswer(value: string): AskUserAnswer | null {
    const q = askQuestion.value;
    if (!q || !value) return null;
    if (askIsPlanApproval.value) {
      return { questionId: q.id, value: "revision_request", notes: value };
    }
    if (q.mode === "confirm") return { questionId: q.id, value: "no", notes: value };
    if (q.mode === "single") {
      return { questionId: q.id, value: OTHER_ANSWER_VALUE, notes: value };
    }
    const picked = new Set(multiPicks.value);
    picked.delete(OTHER_ANSWER_VALUE);
    picked.add(OTHER_ANSWER_VALUE);
    return { questionId: q.id, value: [...picked], notes: value };
  }

  function buildCurrentAskAnswer(): AskUserAnswer | null {
    const text = askQuestion.value?.mode !== "confirm" ? freeformText.value.trim() : "";
    return text ? buildFreeformAnswer(text) : buildAskAnswer();
  }

  function saveNavigableAnswer() {
    const q = askQuestion.value;
    if (!q || q.mode === "confirm" || !canAskSubmit.value) return;
    const ans = buildCurrentAskAnswer();
    if (ans) askAnswers.value[ans.questionId] = ans;
  }

  function advanceAsk(): AskUserResult | null {
    if (askIsLast.value) {
      return { answers: { ...askAnswers.value }, cancelled: false };
    }
    askIndex.value += 1;
    return null;
  }

  function resolveIfDone(result: AskUserResult | null) {
    if (result) resolve(result);
  }

  function confirmAskNo() {
    const q = askQuestion.value;
    if (!q) return;
    const notes = freeformText.value.trim();
    askAnswers.value[q.id] = {
      questionId: q.id,
      value: "no",
      notes: notes || undefined,
    };
    resolveIfDone(advanceAsk());
  }

  function submitAsk() {
    if (!canAskSubmit.value) return;
    const ans = buildCurrentAskAnswer();
    if (!ans) return;
    askAnswers.value[ans.questionId] = ans;
    resolveIfDone(advanceAsk());
  }

  function submitAskFreeform(value = freeformText.value.trim()) {
    const ans = buildFreeformAnswer(value.trim());
    if (!ans) return;
    askAnswers.value[ans.questionId] = ans;
    resolveIfDone(advanceAsk());
  }

  function skipAsk() {
    const q = askQuestion.value;
    if (!q) return;
    delete askAnswers.value[q.id];
    resolveIfDone(advanceAsk());
  }

  function backAsk() {
    if (!canGoPrev.value) return;
    saveNavigableAnswer();
    askIndex.value -= 1;
  }

  function cancelAsk() {
    if (!askDismissable.value) return;
    resolve({ answers: { ...askAnswers.value }, cancelled: true });
  }

  function resetAskState() {
    askIndex.value = 0;
    askAnswers.value = {};
    singleFocus.value = null;
    activeOptionId.value = null;
    singlePick.value = null;
    multiPicks.value = new Set();
    freeformText.value = "";
  }

  watch(askKey, resetAskState, { immediate: true });

  watch(
    () => askQuestion.value?.id,
    (qid) => {
      if (!qid) return;
      const prior = askAnswers.value[qid];
      const q = askQuestion.value!;

      multiPicks.value = new Set();
      singleFocus.value = null;
      activeOptionId.value = null;
      singlePick.value = null;
      freeformText.value = "";

      if (q.mode === "single") {
        if (prior && typeof prior.value === "string") {
          if (prior.value === OTHER_ANSWER_VALUE) {
            freeformText.value = prior.notes ?? "";
          } else {
            singleFocus.value = prior.value;
            singlePick.value = prior.value;
          }
        }
      } else if (q.mode === "multi") {
        if (prior && Array.isArray(prior.value)) {
          if (prior.value.includes(OTHER_ANSWER_VALUE)) {
            multiPicks.value = new Set(
              prior.value.filter((value) => value !== OTHER_ANSWER_VALUE),
            );
            freeformText.value = prior.notes ?? "";
          } else {
            multiPicks.value = new Set(prior.value);
          }
        }
      }
    },
    { immediate: true },
  );

  return {
    askIndex,
    singleFocus,
    activeOptionId,
    singlePick,
    multiPicks,
    askTotal,
    askQuestion,
    askDismissable,
    askIsLast,
    canGoPrev,
    askTitle,
    askIsPlanApproval,
    askOptionsWithId,
    askHasPreview,
    askFocusedOption,
    canAskSubmit,
    focusOption,
    highlightOption,
    clearOptionHighlight,
    selectSingleOption,
    toggleMulti,
    confirmAskNo,
    submitAsk,
    submitAskFreeform,
    skipAsk,
    backAsk,
    cancelAsk,
  };
}
