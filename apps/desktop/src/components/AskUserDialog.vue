<script setup lang="ts">
/**
 * Agent → 用户问询浮层：一次 spec 可含多道题，UI 维护当前 index + 答案 map，
 * 逐题切换；最后一题提交后整体 resolve AskUserResult。
 *
 * 视觉壳复用 .search-palette + .dialog__card，与 ConfirmDialog/CategoryDialog 同体系。
 * 单题、多题共用同一布局：只是步进 chip 和「上一题」按钮在单题时不渲染。
 *
 * 键盘约定：
 * - Esc：dismissable=true 时整体取消
 * - Enter：confirm 模式触发主按钮；single 模式提交当前 focused 选项；multi 模式提交当前已选
 * - ↑/↓：在选项之间移动 focus（带 preview 时同步切换右侧预览）
 * - Space：toggle 多选 / 选中单选
 */
import { computed, nextTick, ref, watch } from "vue";
import { AlertTriangle, ArrowLeft, ArrowRight, Check, X } from "lucide-vue-next";
import type {
  AskUserAnswer,
  AskUserOption,
  AskUserQuestion,
  AskUserResult,
  AskUserSpec,
} from "@lilia/contracts";

const props = defineProps<{ spec: AskUserSpec }>();
const emit = defineEmits<{ resolve: [result: AskUserResult] }>();

const index = ref(0);
const answers = ref<Record<string, AskUserAnswer>>({});
/** 单选当前 focus 的 option id（决定 preview 内容与 Enter 默认选中）。 */
const singleFocus = ref<string | null>(null);
/** 多选已勾的 option id 集合。 */
const multiPicks = ref<Set<string>>(new Set());
/** 选择「其他」时的自填文本。 */
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

/** 当前题是否已具备「可提交」状态。 */
const canSubmit = computed(() => {
  const q = current.value;
  if (!q) return false;
  if (q.mode === "confirm") return true;
  if (q.mode === "single") {
    if (!singleFocus.value) return false;
    if (singleFocus.value === "__other__") return otherText.value.trim().length > 0;
    return true;
  }
  // multi
  const min = q.minSelections ?? 1;
  const picks = [...multiPicks.value];
  if (picks.includes("__other__") && otherText.value.trim().length === 0) return false;
  return picks.length >= min;
});

/** 切题时把界面状态重置为该题已有回答（如果用户回退过）。 */
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
        if (prior.value === "other") {
          singleFocus.value = "__other__";
          otherText.value = prior.notes ?? "";
        }
      } else {
        // 默认 focus 推荐项；没有推荐项就 focus 第一项
        const recommended = optionsWithId.value.find((o) => o.recommended);
        singleFocus.value = (recommended ?? optionsWithId.value[0])?.id ?? null;
      }
    } else if (q.mode === "multi") {
      singleFocus.value = optionsWithId.value[0]?.id ?? null;
      if (prior && Array.isArray(prior.value)) {
        multiPicks.value = new Set(prior.value);
        if (prior.value.includes("other")) {
          multiPicks.value.delete("other");
          multiPicks.value.add("__other__");
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

function ariaHeader(q: AskUserQuestion): string {
  return q.header ? q.header : "";
}

function optionDanger(opt: AskUserOption | { id: string }): boolean {
  return "danger" in opt && opt.danger === true;
}

function focusOption(id: string) {
  singleFocus.value = id;
  if (id === "__other__") {
    nextTick(() => otherInput.value?.focus());
  }
}

function toggleMulti(id: string) {
  const next = new Set(multiPicks.value);
  if (next.has(id)) next.delete(id);
  else {
    const q = current.value!;
    if (q.maxSelections && next.size >= q.maxSelections) {
      // 满了：删一个最早进的 (Set 保持插入序)
      const first = next.values().next().value;
      if (first) next.delete(first);
    }
    next.add(id);
  }
  multiPicks.value = next;
  singleFocus.value = id;
  if (id === "__other__") nextTick(() => otherInput.value?.focus());
}

function pickSingleAndSubmit(id: string) {
  focusOption(id);
  if (id === "__other__") return; // 等用户填完文本再按继续
  submit();
}

function buildAnswer(): AskUserAnswer | null {
  const q = current.value;
  if (!q) return null;
  if (q.mode === "confirm") {
    return { questionId: q.id, value: "yes" };
  }
  if (q.mode === "single") {
    const id = singleFocus.value!;
    if (id === "__other__") {
      return { questionId: q.id, value: "other", notes: otherText.value.trim() };
    }
    return { questionId: q.id, value: id };
  }
  // multi
  const ids = [...multiPicks.value];
  const hasOther = ids.includes("__other__");
  const cleaned = ids.filter((x) => x !== "__other__");
  if (hasOther) cleaned.push("other");
  return {
    questionId: q.id,
    value: cleaned,
    notes: hasOther ? otherText.value.trim() : undefined,
  };
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
  // 跳过的题不进 answers，但 advance 仍要递进
  delete answers.value[q.id];
  advance();
}

function back() {
  if (canGoPrev.value) index.value -= 1;
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
  const allIds = q.allowOther ? [...list.map((o) => o.id), "__other__"] : list.map((o) => o.id);
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
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        v-if="current"
        class="search-palette ask-user"
        role="dialog"
        aria-modal="true"
        :aria-label="titleText"
        tabindex="-1"
        @click.self="cancel"
        @keydown="onKeydown"
      >
        <div
          ref="cardEl"
          class="search-palette__card dialog__card ask-user__card"
          :class="{ 'ask-user__card--with-preview': hasPreview }"
          tabindex="-1"
        >
          <header class="dialog__header ask-user__header" :class="{ 'dialog__header--danger': current.danger }">
            <AlertTriangle v-if="current.danger" :size="14" aria-hidden="true" />
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
                v-if="ariaHeader(current)"
                class="ask-user__chip"
              >{{ ariaHeader(current) }}</span>
              <p class="ask-user__qtext">{{ current.question }}</p>
            </div>

            <!-- confirm 模式：没有 options，主体只有问题文本，下方两枚按钮 -->
            <!-- single / multi 模式：渲染选项列表（左）+ 可选 preview（右） -->
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
                    'is-danger': optionDanger(opt),
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
                    'is-active': singleFocus === '__other__',
                    'is-picked': current.mode === 'multi' && multiPicks.has('__other__'),
                  }"
                >
                  <button
                    type="button"
                    class="ask-user__option-btn"
                    :role="current.mode === 'single' ? 'radio' : 'checkbox'"
                    :aria-checked="current.mode === 'single'
                      ? singleFocus === '__other__'
                      : multiPicks.has('__other__')"
                    @click="current.mode === 'single' ? focusOption('__other__') : toggleMulti('__other__')"
                  >
                    <span class="ask-user__option-indicator" aria-hidden="true">
                      <Check
                        v-if="current.mode === 'multi' && multiPicks.has('__other__')"
                        :size="12"
                      />
                    </span>
                    <span class="ask-user__option-main">
                      <span class="ask-user__option-label">其他…</span>
                      <span class="ask-user__option-desc">自己写一段。</span>
                    </span>
                  </button>
                  <textarea
                    v-if="singleFocus === '__other__' || (current.mode === 'multi' && multiPicks.has('__other__'))"
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

          <footer class="dialog__actions ask-user__actions">
            <button
              v-if="current.skippable !== false && total > 1"
              type="button"
              class="ghost ask-user__skip"
              @click="skip"
            >
              跳过
            </button>
            <span class="ask-user__spacer" />
            <button
              v-if="canGoPrev"
              type="button"
              class="ghost"
              @click="back"
            >
              <ArrowLeft :size="13" aria-hidden="true" />
              上一题
            </button>

            <template v-if="current.mode === 'confirm'">
              <button type="button" class="ghost" @click="confirmAnswerNo">
                {{ current.cancelLabel ?? "不要" }}
              </button>
              <button
                type="button"
                :class="current.danger ? 'ghost danger' : 'primary'"
                @click="submit"
              >
                {{ current.confirmLabel ?? "好的" }}
              </button>
            </template>
            <template v-else>
              <button
                type="button"
                class="primary"
                :disabled="!canSubmit"
                @click="submit"
              >
                {{ isLast ? "完成" : "继续" }}
                <ArrowRight v-if="!isLast" :size="13" aria-hidden="true" />
              </button>
            </template>
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
