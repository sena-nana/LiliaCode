import { computed, onScopeDispose, ref, watch, type ComputedRef } from "vue";
import {
  ASK_USER_CONFIRM_ANSWER_VALUE,
  ASK_USER_SINGLE_SELECT_MODE,
  DEFAULT_ASK_USER_MODE,
  type AskUserAnswer,
  type AskUserOption,
  type AskUserResult,
  type AskUserSpec,
} from "@lilia/contracts";

export const FREE_IMPLEMENTATION_COUNTDOWN_MS = 8000;
export const FREE_IMPLEMENTATION_COUNTDOWN_SECONDS = FREE_IMPLEMENTATION_COUNTDOWN_MS / 1000;

function optionId(option: AskUserOption, index: number): string {
  return option.id ?? option.label ?? `opt-${index}`;
}

function recommendedOptions(options: AskUserOption[] | undefined): string[] {
  return (options ?? [])
    .map((option, index) => option.recommended === true ? optionId(option, index) : null)
    .filter((value): value is string => Boolean(value));
}

function recommendedAnswerForQuestion(
  question: AskUserSpec["questions"][number],
): AskUserAnswer | null {
  if (question.mode === DEFAULT_ASK_USER_MODE) {
    return { questionId: question.id, value: ASK_USER_CONFIRM_ANSWER_VALUE };
  }

  const recommended = recommendedOptions(question.options);
  if (question.mode === ASK_USER_SINGLE_SELECT_MODE) {
    const value = recommended[0];
    return value ? { questionId: question.id, value } : null;
  }

  const min = question.minSelections ?? 1;
  const max = question.maxSelections ?? recommended.length;
  const picked = recommended.slice(0, Math.max(0, max));
  if (picked.length < min) return null;
  return { questionId: question.id, value: picked };
}

export function recommendedAskUserResult(spec: AskUserSpec | null | undefined): AskUserResult | null {
  const questions = spec?.questions ?? [];
  if (questions.length === 0) return null;
  const answers: Record<string, AskUserAnswer> = {};
  for (const question of questions) {
    const answer = recommendedAnswerForQuestion(question);
    if (!answer) return null;
    answers[question.id] = answer;
  }
  return { answers, cancelled: false };
}

export function useFreeImplementationCountdown(options: {
  enabled: ComputedRef<boolean>;
  decisionKey: ComputedRef<string>;
  decisionLabel: () => string;
  runDecision: () => void;
}) {
  const remainingSeconds = ref(0);
  const label = ref("");
  const canceled = ref(false);
  let timerId: number | null = null;
  let tickTimerId: number | null = null;

  const text = computed(() =>
    label.value && remainingSeconds.value > 0
      ? `自由实现将在 ${remainingSeconds.value} 秒后${label.value}`
      : "",
  );

  function clearTimers() {
    if (timerId !== null) {
      window.clearTimeout(timerId);
      timerId = null;
    }
    if (tickTimerId !== null) {
      window.clearInterval(tickTimerId);
      tickTimerId = null;
    }
    remainingSeconds.value = 0;
    label.value = "";
  }

  function start() {
    clearTimers();
    if (!options.enabled.value || canceled.value || !options.decisionKey.value) return;
    const nextLabel = options.decisionLabel();
    if (!nextLabel) return;
    label.value = nextLabel;
    remainingSeconds.value = FREE_IMPLEMENTATION_COUNTDOWN_SECONDS;
    const deadline = Date.now() + FREE_IMPLEMENTATION_COUNTDOWN_MS;
    tickTimerId = window.setInterval(() => {
      remainingSeconds.value = Math.max(1, Math.ceil((deadline - Date.now()) / 1000));
    }, 250);
    timerId = window.setTimeout(() => {
      clearTimers();
      if (!options.enabled.value || canceled.value || !options.decisionKey.value) return;
      options.runDecision();
    }, FREE_IMPLEMENTATION_COUNTDOWN_MS);
  }

  function cancel() {
    canceled.value = true;
    clearTimers();
  }

  function reset() {
    canceled.value = false;
  }

  watch([options.enabled, options.decisionKey, canceled], start, { immediate: true });
  onScopeDispose(clearTimers);

  return {
    text,
    cancel,
    reset,
  };
}

