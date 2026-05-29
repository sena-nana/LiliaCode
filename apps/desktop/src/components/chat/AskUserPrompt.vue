<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { AlertTriangle, ArrowLeft, ArrowRight, Check, CircleHelp, X } from "lucide-vue-next";
import type {
  AskUserAnswer,
  AskUserOption,
  AskUserQuestion,
  AskUserResult,
  AskUserSpec,
} from "@lilia/contracts";

const props = defineProps<{ spec: AskUserSpec }>();
const emit = defineEmits<{ resolve: [result: AskUserResult] }>();

const OTHER_OPTION_ID = "__other__";
const OTHER_ANSWER_VALUE = "other";

const index = ref(0);
const answers = ref<Record<string, AskUserAnswer>>({});
const singleFocus = ref<string | null>(null);
const multiPicks = ref<Set<string>>(new Set());
const otherText = ref("");

const otherInput = ref<HTMLTextAreaElement | null>(null);
const cardEl = ref<HTMLElement | null>(null);

const total = computed(() => props.spec.questions.length);
const current = computed<AskUserQuestion | null>(() => props.spec.questions[index.value] ?? null);
const dismissable = computed(() => props.spec.dismissable !== false);

const titleText = computed(() => {
  if (props.spec.title) return props.spec.title;
  return total.value > 1 ? `Lilia 想确认 ${total.value} 件事` : "Lilia 想确认一下";
});

const optionsWithId = computed<(AskUserOption & { id: string })[]>(() => {
  const q = current.value;
  if (!q || !q.options) return [];
  return q.options.map((opt, i) => ({ ...opt, id: opt.id ?? opt.label ?? `opt-${i}` }));
});

const hasPreview = computed(() => optionsWithId.value.some((opt) => !!opt.preview));

const focusedOption = computed(() =>
  optionsWithId.value.find((opt) => opt.id === singleFocus.value) ?? null,
);

const canGoPrev = computed(() => index.value > 0);
const isLast = computed(() => index.value >= total.value - 1);

const canSubmit = computed(() => {
  const q = current.value;
  if (!q) return false;
  if (q.mode === "confirm") return true;
  if (q.mode === "single") {
    if (!singleFocus.value) return false;
    if (singleFocus.value === OTHER_OPTION_ID) return otherText.value.trim().length > 0;
    return true;
  }
  const min = q.minSelections ?? 1;
  const picks = [...multiPicks.value];
  if (picks.includes(OTHER_OPTION_ID) && otherText.value.trim().length === 0) return false;
  return picks.length >= min;
});

watch(
  () => current.value?.id,
  (qid) => {
    if (!qid) return;
    const prior = answers.value[qid];
    const q = current.value!;

    multiPicks.value = new Set();
    otherText.value = "";

    if (q.mode === "single") {
      if (prior && typeof prior.value === "string") {
        singleFocus.value = prior.value;
        if (prior.value === OTHER_ANSWER_VALUE) {
          singleFocus.value = OTHER_OPTION_ID;
          otherText.value = prior.notes ?? "";
        }
      } else {
        const recommended = optionsWithId.value.find((o) => o.recommended);
        singleFocus.value = (recommended ?? optionsWithId.value[0])?.id ?? null;
      }
    } else if (q.mode === "multi") {
      singleFocus.value = optionsWithId.value[0]?.id ?? null;
      if (prior && Array.isArray(prior.value)) {
        multiPicks.value = new Set(prior.value);
        if (prior.value.includes(OTHER_ANSWER_VALUE)) {
          multiPicks.value.delete(OTHER_ANSWER_VALUE);
          multiPicks.value.add(OTHER_OPTION_ID);
          otherText.value = prior.notes ?? "";
        }
      }
    } else {
      singleFocus.value = null;
    }

    nextTick(() => cardEl.value?.focus());
  },
  { immediate: true },
);

function focusOption(id: string) {
  singleFocus.value = id;
  if (id === OTHER_OPTION_ID) {
    nextTick(() => otherInput.value?.focus());
  }
}

function toggleMulti(id: string) {
  const next = new Set(multiPicks.value);
  if (next.has(id)) next.delete(id);
  else {
    const q = current.value!;
    if (q.maxSelections && next.size >= q.maxSelections) {
      const first = next.values().next();
      if (!first.done) next.delete(first.value);
    }
    next.add(id);
  }
  multiPicks.value = next;
  singleFocus.value = id;
  if (id === OTHER_OPTION_ID) nextTick(() => otherInput.value?.focus());
}

function pickSingleAndSubmit(id: string) {
  focusOption(id);
  if (id !== OTHER_OPTION_ID) submit();
}

function buildAnswer(): AskUserAnswer | null {
  const q = current.value;
  if (!q) return null;
  if (q.mode === "confirm") {
    return { questionId: q.id, value: "yes" };
  }
  if (q.mode === "single") {
    const id = singleFocus.value!;
    if (id === OTHER_OPTION_ID) {
      return { questionId: q.id, value: OTHER_ANSWER_VALUE, notes: otherText.value.trim() };
    }
    return { questionId: q.id, value: id };
  }
  const ids = [...multiPicks.value];
  const hasOther = ids.includes(OTHER_OPTION_ID);
  const cleaned = ids.filter((x) => x !== OTHER_OPTION_ID);
  if (hasOther) cleaned.push(OTHER_ANSWER_VALUE);
  return {
    questionId: q.id,
    value: cleaned,
    notes: hasOther ? otherText.value.trim() : undefined,
  };
}

function saveNavigableAnswer() {
  const q = current.value;
  if (!q || q.mode === "confirm" || !canSubmit.value) return;
  const ans = buildAnswer();
  if (ans) answers.value[ans.questionId] = ans;
}

function confirmAnswerNo() {
  const q = current.value;
  if (!q) return;
  answers.value[q.id] = { questionId: q.id, value: "no" };
  advance();
}

function submit() {
  if (!canSubmit.value) return;
  const ans = buildAnswer();
  if (!ans) return;
  answers.value[ans.questionId] = ans;
  advance();
}

function skip() {
  const q = current.value;
  if (!q) return;
  delete answers.value[q.id];
  advance();
}

function back() {
  if (!canGoPrev.value) return;
  saveNavigableAnswer();
  index.value -= 1;
}

function advance() {
  if (isLast.value) {
    emit("resolve", { answers: { ...answers.value }, cancelled: false });
    return;
  }
  index.value += 1;
}

function cancel() {
  if (!dismissable.value) return;
  emit("resolve", { answers: { ...answers.value }, cancelled: true });
}

function onKeydown(e: KeyboardEvent) {
  const q = current.value;
  if (!q) return;
  if (e.key === "Escape" && dismissable.value) {
    e.preventDefault();
    cancel();
    return;
  }
  if (e.key === "Enter" && !e.shiftKey) {
    if (e.target instanceof HTMLTextAreaElement) return;
    e.preventDefault();
    submit();
    return;
  }
  if (q.mode === "confirm") return;
  const list = optionsWithId.value;
  const allIds = q.allowOther ? [...list.map((o) => o.id), OTHER_OPTION_ID] : list.map((o) => o.id);
  if (allIds.length === 0) return;
  const cur = singleFocus.value ?? allIds[0];
  const i = allIds.indexOf(cur);
  if (e.key === "ArrowDown") {
    e.preventDefault();
    focusOption(allIds[(i + 1) % allIds.length]);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    focusOption(allIds[(i - 1 + allIds.length) % allIds.length]);
  } else if (e.key === " " && q.mode === "multi") {
    e.preventDefault();
    toggleMulti(cur);
  }
}
</script>

<template>
  <Transition name="ask-user">
    <section
      v-if="current"
      ref="cardEl"
      class="ask-user"
      :class="{ 'ask-user--danger': current.danger }"
      role="region"
      aria-live="assertive"
      :aria-label="titleText"
      tabindex="-1"
      @keydown="onKeydown"
    >
      <header class="ask-user__header">
        <span class="ask-user__icon" aria-hidden="true">
          <AlertTriangle v-if="current.danger" :size="14" />
          <CircleHelp v-else :size="14" />
        </span>
        <span class="ask-user__title">{{ titleText }}</span>
        <span v-if="spec.source" class="ask-user__source">{{ spec.source }}</span>
        <span v-if="total > 1" class="ask-user__progress" aria-live="polite">
          {{ index + 1 }} / {{ total }}
        </span>
        <button
          v-if="dismissable"
          type="button"
          class="ask-user__close"
          aria-label="关闭"
          @click="cancel"
        >
          <X :size="14" />
        </button>
      </header>

      <div class="ask-user__body">
        <div class="ask-user__question">
          <span
            v-if="current.header"
            class="ask-user__chip"
          >{{ current.header }}</span>
          <p class="ask-user__qtext">{{ current.question }}</p>
        </div>

        <div
          v-if="current.mode !== 'confirm'"
          class="ask-user__main"
          :class="{ 'ask-user__main--with-preview': hasPreview }"
        >
          <ul
            class="ask-user__options"
            :role="current.mode === 'single' ? 'radiogroup' : 'group'"
          >
            <li
              v-for="opt in optionsWithId"
              :key="opt.id"
              class="ask-user__option"
              :class="{
                'is-active': singleFocus === opt.id,
                'is-picked': current.mode === 'multi' && multiPicks.has(opt.id),
                'is-recommended': opt.recommended,
                'is-danger': opt.danger,
              }"
            >
              <button
                type="button"
                class="ask-user__option-btn"
                :role="current.mode === 'single' ? 'radio' : 'checkbox'"
                :aria-checked="current.mode === 'single'
                  ? singleFocus === opt.id
                  : multiPicks.has(opt.id)"
                @mouseenter="focusOption(opt.id)"
                @focus="focusOption(opt.id)"
                @click="current.mode === 'single' ? pickSingleAndSubmit(opt.id) : toggleMulti(opt.id)"
              >
                <span class="ask-user__option-indicator" aria-hidden="true">
                  <Check
                    v-if="current.mode === 'multi' && multiPicks.has(opt.id)"
                    :size="12"
                  />
                </span>
                <span class="ask-user__option-main">
                  <span class="ask-user__option-label">
                    {{ opt.label }}
                    <span v-if="opt.recommended" class="ask-user__badge">推荐</span>
                  </span>
                  <span
                    v-if="opt.description"
                    class="ask-user__option-desc"
                  >{{ opt.description }}</span>
                </span>
              </button>
            </li>

            <li
              v-if="current.allowOther"
              class="ask-user__option ask-user__option--other"
              :class="{
                'is-active': singleFocus === OTHER_OPTION_ID,
                'is-picked': current.mode === 'multi' && multiPicks.has(OTHER_OPTION_ID),
              }"
            >
              <button
                type="button"
                class="ask-user__option-btn"
                :role="current.mode === 'single' ? 'radio' : 'checkbox'"
                :aria-checked="current.mode === 'single'
                  ? singleFocus === OTHER_OPTION_ID
                  : multiPicks.has(OTHER_OPTION_ID)"
                @click="current.mode === 'single' ? focusOption(OTHER_OPTION_ID) : toggleMulti(OTHER_OPTION_ID)"
              >
                <span class="ask-user__option-indicator" aria-hidden="true">
                  <Check
                    v-if="current.mode === 'multi' && multiPicks.has(OTHER_OPTION_ID)"
                    :size="12"
                  />
                </span>
                <span class="ask-user__option-main">
                  <span class="ask-user__option-label">其他…</span>
                  <span class="ask-user__option-desc">自己写一段。</span>
                </span>
              </button>
              <textarea
                v-if="singleFocus === OTHER_OPTION_ID || (current.mode === 'multi' && multiPicks.has(OTHER_OPTION_ID))"
                ref="otherInput"
                v-model="otherText"
                class="ask-user__other-input text-input"
                rows="2"
                placeholder="详细写一下你的回答…"
              />
            </li>
          </ul>

          <aside
            v-if="hasPreview"
            class="ask-user__preview"
            aria-label="选项预览"
          >
            <pre v-if="focusedOption?.preview" class="ask-user__preview-pre">{{ focusedOption.preview }}</pre>
            <p v-else class="ask-user__preview-empty">
              把鼠标移到选项上 / 用方向键聚焦，这里会显示对比预览。
            </p>
          </aside>
        </div>
      </div>

      <footer class="ask-user__actions">
        <button
          v-if="current.skippable !== false && total > 1"
          type="button"
          class="ghost ask-user__skip ask-user__btn"
          @click="skip"
        >
          跳过
        </button>
        <span class="ask-user__spacer" />
        <button
          v-if="canGoPrev"
          type="button"
          class="ghost ask-user__btn"
          @click="back"
        >
          <ArrowLeft :size="13" aria-hidden="true" />
          上一题
        </button>

        <template v-if="current.mode === 'confirm'">
          <button type="button" class="ghost ask-user__btn" @click="confirmAnswerNo">
            {{ current.cancelLabel ?? "不要" }}
          </button>
          <button
            type="button"
            class="ask-user__btn"
            :class="current.danger ? 'ghost danger' : 'primary'"
            @click="submit"
          >
            {{ current.confirmLabel ?? "好的" }}
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            class="primary ask-user__btn"
            :disabled="!canSubmit"
            @click="submit"
          >
            {{ isLast ? "完成" : "继续" }}
            <ArrowRight v-if="!isLast" :size="13" aria-hidden="true" />
          </button>
        </template>
      </footer>
    </section>
  </Transition>
</template>
