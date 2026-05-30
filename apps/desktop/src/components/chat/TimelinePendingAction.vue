<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Check,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  X,
} from "lucide-vue-next";
import type { AskUserResult } from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type { ToolConsentDecision } from "../../services/chat";

const props = defineProps<{
  action: PendingAgentAction;
}>();

const emit = defineEmits<{
  resolve: [resolution: PendingAgentActionResolution];
}>();

const freeformText = ref("");
const toolExpanded = ref(false);
const toolMessage = ref("");
const toolSubmitting = ref<ToolConsentDecision | null>(null);

const actionKey = computed(() =>
  props.action.kind === "tool_consent"
    ? `tool:${props.action.requestId}`
    : `ask:${props.action.ask.id}`,
);
const activeAsk = computed(() =>
  props.action.kind === "tool_consent" ? null : props.action.ask,
);
const {
  askIndex,
  askTotal,
  askQuestion,
  askDismissable,
  askIsLast,
  canGoPrev,
  askTitle,
  askOptionsWithId,
  canAskSubmit,
  activeOptionId,
  singlePick,
  multiPicks,
  focusOption,
  highlightOption,
  clearOptionHighlight,
  selectSingleOption,
  toggleMulti,
  submitAsk,
  submitAskFreeform,
  confirmAskNo,
  skipAsk,
  backAsk,
  cancelAsk,
} = useAskUserInteraction(activeAsk, freeformText, resolveAsk);

const toolRequest = computed(() =>
  props.action.kind === "tool_consent" ? props.action.request : null,
);
const { toolDanger, toolIcon, toolHeadline, toolInputJson, toolSubtitle } =
  useToolConsentPresentation(toolRequest);

function resolveAsk(result: AskUserResult) {
  if (props.action.kind === "tool_consent") return;
  emit("resolve", {
    kind: props.action.kind,
    requestId: props.action.requestId,
    askId: props.action.ask.id,
    result,
  });
}

function decideTool(decision: ToolConsentDecision) {
  if (props.action.kind !== "tool_consent" || toolSubmitting.value) return;
  toolSubmitting.value = decision;
  emit("resolve", {
    kind: "tool_consent",
    requestId: props.action.requestId,
    decision,
    message: decision === "deny"
      ? toolMessage.value.trim() || "用户拒绝了此次工具调用"
      : undefined,
  });
}

watch(actionKey, () => {
  toolExpanded.value = false;
  toolMessage.value = "";
  toolSubmitting.value = null;
}, { immediate: true });
</script>

<template>
  <section
    v-if="props.action.kind === 'tool_consent' && toolRequest"
    class="timeline-pending-action composer-inline composer-inline--tool"
    :class="{ 'composer-inline--danger': toolDanger, 'is-expanded': toolExpanded }"
    role="alert"
    aria-live="assertive"
  >
    <div class="composer-inline__tool-row">
      <span class="composer-inline__icon" aria-hidden="true">
        <AlertTriangle v-if="toolDanger" :size="14" />
        <component v-else :is="toolIcon" :size="14" />
      </span>
      <div class="composer-inline__tool-main">
        <div class="composer-inline__tool-head">
          <span class="composer-inline__tool-name">{{ toolRequest.toolName }}</span>
          <span class="composer-inline__headline">{{ toolHeadline }}</span>
        </div>
        <p v-if="toolSubtitle" class="composer-inline__subtitle">{{ toolSubtitle }}</p>
      </div>
      <button
        v-if="toolInputJson && toolInputJson !== '{}'"
        type="button"
        class="composer-inline__toggle"
        :aria-expanded="toolExpanded"
        @click="toolExpanded = !toolExpanded"
      >
        <component :is="toolExpanded ? ChevronDown : ChevronRight" :size="12" aria-hidden="true" />
        {{ toolExpanded ? "收起" : "查看入参" }}
      </button>
    </div>
    <pre v-if="toolExpanded" class="composer-inline__details">{{ toolInputJson }}</pre>
    <div class="timeline-pending-action__row">
      <textarea
        v-model="toolMessage"
        class="timeline-pending-action__input"
        rows="1"
        placeholder="拒绝理由"
      />
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ghost composer-inline__btn"
          :disabled="toolSubmitting !== null"
          @click="decideTool('deny')"
        >
          {{ toolSubmitting === "deny" ? "处理中..." : "忽略" }}
        </button>
        <button
          type="button"
          class="primary composer-inline__btn"
        :disabled="toolSubmitting !== null"
        @click="decideTool('allow')"
      >
          {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
      </button>
      </div>
    </div>
  </section>

  <section
    v-else-if="props.action.kind === 'plan_approval'"
    class="timeline-pending-action timeline-pending-action--plan"
    role="region"
    :aria-label="askTitle"
  >
    <textarea
      v-model="freeformText"
      class="timeline-pending-action__input"
      rows="2"
      placeholder="修改要求"
    />
    <div class="composer-inline__actions">
      <button
        type="button"
        class="ghost composer-inline__btn"
        :disabled="!freeformText.trim()"
        @click="submitAskFreeform()"
      >
        提交修改要求
      </button>
      <button type="button" class="primary composer-inline__btn" @click="submitAsk">
        同意
      </button>
    </div>
  </section>

  <section
    v-else-if="activeAsk && askQuestion"
    class="timeline-pending-action composer-inline composer-inline--ask"
    :class="{ 'composer-inline--danger': askQuestion.danger }"
    role="region"
    aria-live="assertive"
    :aria-label="askTitle"
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
        @click="cancelAsk"
      >
        <X :size="14" aria-hidden="true" />
      </button>
    </header>

    <div class="composer-inline__body">
      <div class="composer-inline__question">
        <span v-if="askQuestion.header" class="composer-inline__chip">
          {{ askQuestion.header }}
        </span>
        <p class="composer-inline__qtext">{{ askQuestion.question }}</p>
      </div>

      <ul
        v-if="askQuestion.mode !== 'confirm'"
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
            @mouseenter="highlightOption(opt.id)"
            @mouseleave="clearOptionHighlight(opt.id)"
            @focus="focusOption(opt.id)"
            @click="askQuestion.mode === 'single' ? selectSingleOption(opt.id) : toggleMulti(opt.id)"
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

      <textarea
        v-if="askQuestion.mode !== 'confirm'"
        v-model="freeformText"
        class="timeline-pending-action__input"
        rows="1"
        placeholder="自定义回答"
      />
    </div>

    <footer class="composer-inline__actions">
      <button
        v-if="askQuestion.skippable !== false && askTotal > 1"
        type="button"
        class="ghost composer-inline__skip composer-inline__btn"
        @click="skipAsk"
      >
        跳过
      </button>
      <span class="composer-inline__spacer" />
      <button
        v-if="canGoPrev"
        type="button"
        class="ghost composer-inline__btn"
        @click="backAsk"
      >
        <ArrowLeft :size="13" aria-hidden="true" />
        上一题
      </button>
      <button
        v-if="askQuestion.mode === 'confirm'"
        type="button"
        class="ghost composer-inline__btn"
        @click="confirmAskNo"
      >
        {{ askQuestion.cancelLabel ?? "不要" }}
      </button>
      <button
        type="button"
        class="composer-inline__btn"
        :class="askQuestion.danger ? 'ghost danger' : 'primary'"
        :disabled="askQuestion.mode !== 'confirm' && !canAskSubmit"
        @click="submitAsk"
      >
        {{ askQuestion.mode === "confirm" ? (askQuestion.confirmLabel ?? "好的") : askIsLast ? "完成" : "继续" }}
        <ArrowRight v-if="askQuestion.mode !== 'confirm' && !askIsLast" :size="13" aria-hidden="true" />
      </button>
    </footer>
  </section>
</template>
