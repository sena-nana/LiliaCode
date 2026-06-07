<script setup lang="ts">
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Check,
  CircleHelp,
  X,
} from "lucide-vue-next";
import type { AskUserOption, AskUserQuestion } from "@lilia/contracts";
import type { PendingAsk } from "../../composables/useAskUser";

type AskOptionView = AskUserOption & { id: string };

withDefaults(defineProps<{
  activeAsk: PendingAsk;
  activeOptionId?: string | null;
  askDismissable: boolean;
  askFocusedOption?: AskOptionView | null;
  askHasPreview: boolean;
  askIndex: number;
  askIsLast: boolean;
  askIsPlanApproval?: boolean;
  askOptionsWithId: AskOptionView[];
  askOtherSelected?: boolean;
  askQuestion: AskUserQuestion;
  askTitle: string;
  askTotal: number;
  canAskSubmit: boolean;
  canGoPrev: boolean;
  actionsBlocked?: boolean;
  freeformText?: string;
  inputClass?: string;
  inputPlaceholder?: string;
  multiPicks: Set<string>;
  rootClass?: string;
  showChoiceFooter?: boolean;
  showConfirmFooter?: boolean;
  singlePick?: string | null;
  tabindex?: string | number;
}>(), {
  activeOptionId: null,
  askFocusedOption: null,
  askIsPlanApproval: false,
  askOtherSelected: false,
  actionsBlocked: false,
  freeformText: "",
  inputClass: "composer-inline__other-input",
  inputPlaceholder: "自定义回答",
  rootClass: "composer-inline composer-inline--ask",
  showChoiceFooter: false,
  showConfirmFooter: false,
  singlePick: null,
  tabindex: undefined,
});

const emit = defineEmits<{
  backAsk: [];
  cancelAsk: [];
  clearOptionHighlight: [id: string];
  confirmAskNo: [];
  focusOption: [id: string];
  highlightOption: [id: string];
  keydown: [event: KeyboardEvent];
  selectSingleOption: [id: string];
  skipAsk: [];
  submitAsk: [];
  toggleMulti: [id: string];
  "update:freeformText": [value: string];
}>();
</script>

<template>
  <section
    :class="[
      rootClass,
      {
        'composer-inline--danger': askQuestion.danger,
        'composer-inline--plan': askIsPlanApproval,
      },
    ]"
    role="region"
    aria-live="assertive"
    :aria-label="askTitle"
    :tabindex="tabindex"
    @keydown="emit('keydown', $event)"
  >
    <header class="composer-inline__header">
      <span class="composer-inline__icon" aria-hidden="true">
        <AlertTriangle v-if="askQuestion.danger" :size="14" />
        <CircleHelp v-else :size="14" />
      </span>
      <span class="composer-inline__title">{{ askTitle }}</span>
      <span v-if="activeAsk.spec.source" class="composer-inline__source">
        {{ activeAsk.spec.source }}
      </span>
      <span v-if="askTotal > 1" class="composer-inline__progress" aria-live="polite">
        {{ askIndex + 1 }} / {{ askTotal }}
      </span>
      <button
        v-if="askDismissable"
        type="button"
        class="composer-inline__close"
        aria-label="关闭"
        :disabled="actionsBlocked"
        @click="emit('cancelAsk')"
      >
        <X :size="14" aria-hidden="true" />
      </button>
    </header>

    <div v-if="!askIsPlanApproval" class="composer-inline__body">
      <div class="composer-inline__question">
        <span v-if="askQuestion.header" class="composer-inline__chip">
          {{ askQuestion.header }}
        </span>
        <p class="composer-inline__qtext">{{ askQuestion.question }}</p>
      </div>

      <div
        v-if="askQuestion.mode !== 'confirm'"
        class="composer-inline__main"
        :class="{ 'composer-inline__main--with-preview': askHasPreview }"
      >
        <ul
          class="composer-inline__options"
          :role="askQuestion.mode === 'single' ? 'radiogroup' : 'group'"
        >
          <li
            v-for="opt in askOptionsWithId"
            :key="opt.id"
            class="composer-inline__option"
            :class="{
              'is-active': activeOptionId === opt.id,
              'is-picked': askQuestion.mode === 'single'
                ? singlePick === opt.id
                : multiPicks.has(opt.id),
              'is-recommended': opt.recommended,
              'is-danger': opt.danger,
            }"
          >
            <button
              type="button"
              class="composer-inline__option-btn"
              :role="askQuestion.mode === 'single' ? 'radio' : 'checkbox'"
              :aria-checked="askQuestion.mode === 'single'
                ? singlePick === opt.id
                : multiPicks.has(opt.id)"
              @mouseenter="emit('highlightOption', opt.id)"
              @mouseleave="emit('clearOptionHighlight', opt.id)"
              @focus="emit('focusOption', opt.id)"
              @click="askQuestion.mode === 'single'
                ? emit('selectSingleOption', opt.id)
                : emit('toggleMulti', opt.id)"
            >
              <span class="composer-inline__option-indicator" aria-hidden="true">
                <Check v-if="askQuestion.mode === 'multi' && multiPicks.has(opt.id)" :size="12" />
              </span>
              <span class="composer-inline__option-main">
                <span class="composer-inline__option-label">
                  {{ opt.label }}
                  <span v-if="opt.recommended" class="composer-inline__badge">推荐</span>
                </span>
                <span v-if="opt.description" class="composer-inline__option-desc">
                  {{ opt.description }}
                </span>
              </span>
            </button>
          </li>
        </ul>

        <aside
          v-if="askHasPreview"
          class="composer-inline__preview"
          aria-label="选项预览"
        >
          <pre v-if="askFocusedOption?.preview" class="composer-inline__preview-pre">{{ askFocusedOption.preview }}</pre>
          <p v-else class="composer-inline__preview-empty">
            把鼠标移到选项上 / 用方向键聚焦，这里会显示对比预览。
          </p>
        </aside>
      </div>
    </div>

    <footer
      v-if="showConfirmFooter && askQuestion.mode === 'confirm' && !askIsPlanApproval"
      class="composer-inline__actions"
    >
      <button
        v-if="askQuestion.skippable !== false && askTotal > 1"
        type="button"
        class="ghost composer-inline__skip composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('skipAsk')"
      >
        跳过
      </button>
      <span class="composer-inline__spacer" />
      <button
        v-if="canGoPrev"
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('backAsk')"
      >
        <ArrowLeft :size="13" aria-hidden="true" />
        上一题
      </button>

      <button
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('confirmAskNo')"
      >
        {{ askQuestion.cancelLabel ?? "不要" }}
      </button>
      <button
        type="button"
        class="composer-inline__btn"
        :class="askQuestion.danger ? 'ghost danger' : 'primary'"
        :disabled="actionsBlocked"
        @click="emit('submitAsk')"
      >
        {{ askQuestion.confirmLabel ?? "好的" }}
      </button>
    </footer>

    <footer v-if="showChoiceFooter" class="composer-inline__actions">
      <button
        v-if="askQuestion.skippable !== false && askTotal > 1"
        type="button"
        class="ghost composer-inline__skip composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('skipAsk')"
      >
        跳过
      </button>
      <span v-if="!askOtherSelected" class="composer-inline__spacer" />
      <button
        v-if="canGoPrev"
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('backAsk')"
      >
        <ArrowLeft :size="13" aria-hidden="true" />
        上一题
      </button>
      <textarea
        v-if="askQuestion.mode !== 'confirm' && askOtherSelected"
        :value="freeformText"
        :class="inputClass"
        rows="1"
        :placeholder="inputPlaceholder"
        @input="emit('update:freeformText', ($event.target as HTMLTextAreaElement).value)"
      />
      <button
        v-if="askQuestion.mode === 'confirm'"
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('confirmAskNo')"
      >
        {{ askQuestion.cancelLabel ?? "不要" }}
      </button>
      <button
        type="button"
        class="composer-inline__btn"
        :class="askQuestion.danger ? 'ghost danger' : 'primary'"
        :disabled="actionsBlocked || (askQuestion.mode !== 'confirm' && !canAskSubmit)"
        @click="emit('submitAsk')"
      >
        {{ askQuestion.mode === "confirm" ? (askQuestion.confirmLabel ?? "好的") : askIsLast ? "完成" : "继续" }}
        <ArrowRight v-if="askQuestion.mode !== 'confirm' && !askIsLast" :size="13" aria-hidden="true" />
      </button>
    </footer>
  </section>
</template>
