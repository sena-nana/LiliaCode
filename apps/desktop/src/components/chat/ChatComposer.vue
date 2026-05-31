<script setup lang="ts">
/**
 * Composer：textarea 自动撑高（最多 3 行）+ 一排 chip。
 * 挂起态会把工具授权、Agent 提问和计划确认收进输入框内部。
 */

import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  ListChecks,
  Paperclip,
  ShieldCheck,
  Square,
  X,
} from "lucide-vue-next";
import type {
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
  PermissionMode,
} from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import type { PendingAsk } from "../../composables/useAskUser";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
} from "../../services/chat";
import Dropdown from "../Dropdown.vue";

const props = defineProps<{
  state: ChatComposerState;
  attachments?: ChatAttachment[];
  /** 上一轮还在 streaming 时为 true，发送会进入调度队列。 */
  sending?: boolean;
  pendingAsk?: PendingAsk | null;
  toolConsent?: ToolConsentRequest | null;
}>();

const emit = defineEmits<{
  send: [content: string, attachments: ChatAttachment[]];
  "update:state": [next: ChatComposerState];
  "remove-attachment": [attachmentId: string];
  "pick-attachments": [];
  "resolve-ask-user": [result: AskUserResult];
  "resolve-tool-consent": [decision: ToolConsentDecision, message?: string];
  interrupt: [];
}>();

const COMPOSER_INPUT_LINE_HEIGHT = 22;
const COMPOSER_INPUT_VERTICAL_PADDING = 8;
const COMPOSER_INPUT_MAX_ROWS = 3;
const COMPOSER_INPUT_MIN_HEIGHT = COMPOSER_INPUT_LINE_HEIGHT + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_MAX_HEIGHT =
  COMPOSER_INPUT_LINE_HEIGHT * COMPOSER_INPUT_MAX_ROWS + COMPOSER_INPUT_VERTICAL_PADDING;
const COMPOSER_INPUT_TRANSITION_MS = 160;

const messageText = ref("");
const pendingText = ref("");
const textarea = ref<HTMLTextAreaElement | null>(null);
const textareaMeasure = ref<HTMLTextAreaElement | null>(null);
let resizeFrameId: number | null = null;
let overflowTimerId: number | null = null;

const toolExpanded = ref(false);
const toolSubmitting = ref<ToolConsentDecision | null>(null);

const activeAsk = computed(() => props.pendingAsk ?? null);
const activeToolConsent = computed(() =>
  activeAsk.value ? null : props.toolConsent ?? null,
);
const hasPending = computed(() => !!activeAsk.value || !!activeToolConsent.value);
const pendingKey = computed(() => {
  if (activeAsk.value) return `ask:${activeAsk.value.id}`;
  if (activeToolConsent.value) return `tool:${activeToolConsent.value.requestId}`;
  return "none";
});

const inputValue = computed({
  get: () => hasPending.value ? pendingText.value : messageText.value,
  set: (value: string) => {
    if (hasPending.value) pendingText.value = value;
    else messageText.value = value;
  },
});

const {
  askIndex,
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
  askOtherSelected,
  canAskSubmit,
  singleFocus,
  activeOptionId: activeAskOptionId,
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
} = useAskUserInteraction(
  activeAsk,
  pendingText,
  (result) => emit("resolve-ask-user", result),
);

const hasPendingPanel = computed(() =>
  !!(activeAsk.value && askQuestion.value) || !!activeToolConsent.value,
);
const askUsesInputActions = computed(() =>
  !!activeAsk.value && askQuestion.value?.mode !== "confirm",
);
const pendingInputText = computed(() => pendingText.value.trim());
const hasPendingInputText = computed(() => pendingInputText.value.length > 0);
const pendingEntryActionsKey = computed(() => {
  if (askUsesInputActions.value) return "ask-input";
  if (askIsPlanApproval.value) return "ask-plan";
  if (activeAsk.value) return "ask-confirm";
  if (activeToolConsent.value) return "tool";
  return "none";
});

const {
  toolDanger,
  toolIcon,
  toolHeadline,
  toolInlinePreview,
  toolInputJson,
  toolSubtitle,
} = useToolConsentPresentation(activeToolConsent);

const inputPlaceholder = computed(() => {
  if (activeToolConsent.value) return "输入拒绝理由，Enter 拒绝此次调用";
  if (activeAsk.value) {
    const q = askQuestion.value;
    if (askIsPlanApproval.value) return "输入修改要求，Enter 退回计划";
    if (q?.mode === "confirm") return "输入取消原因，Enter 返回给 Agent";
    return "补充其他回答";
  }
  return "可向 agent 询问任何事，输入 @ 使用插件或提及文件";
});

const canSend = computed(() => {
  if (activeToolConsent.value || activeAsk.value) return pendingText.value.trim().length > 0;
  return messageText.value.trim().length > 0 || (props.attachments?.length ?? 0) > 0;
});

const canInterrupt = computed(() =>
  props.sending === true &&
  !hasPending.value &&
  !canSend.value,
);

const canSubmitEntry = computed(() => canSend.value || canInterrupt.value);

const sendTitle = computed(() => {
  if (activeToolConsent.value) return "发送拒绝备注（Enter）";
  if (activeAsk.value) {
    if (askIsPlanApproval.value) return "发送计划修改要求（Enter）";
    return "发送取消原因（Enter）";
  }
  if (canInterrupt.value) return "打断 Agent";
  return props.sending ? "加入调度队列（Enter）" : "发送（Enter）";
});

const sendAriaLabel = computed(() => {
  if (activeToolConsent.value) return "发送拒绝备注";
  if (activeAsk.value) {
    if (askIsPlanApproval.value) return "发送计划修改要求";
    return "发送取消原因";
  }
  if (canInterrupt.value) return "打断 Agent";
  return props.sending ? "加入调度队列" : "发送";
});

const permissionOptions = [
  { value: "full" as PermissionMode, label: "完全访问", hint: "无需逐条确认" },
  { value: "ask" as PermissionMode, label: "询问", hint: "敏感操作前询问" },
  { value: "readonly" as PermissionMode, label: "只读", hint: "禁止写操作" },
];

function patch(next: Partial<ChatComposerState>) {
  emit("update:state", { ...props.state, ...next });
}

function setPermission(v: PermissionMode) { patch({ permission: v }); }
function togglePlanMode() { patch({ planMode: !props.state.planMode }); }

function send() {
  const value = inputValue.value.trim();
  if (activeToolConsent.value) {
    if (!value) return;
    decideToolConsent("deny", value);
    return;
  }
  if (activeAsk.value) {
    if (askUsesInputActions.value) submitAsk();
    else submitAskFreeform(value);
    return;
  }

  const attachments = props.attachments ?? [];
  if (!value && attachments.length === 0) return;
  emit("send", value, attachments);
  messageText.value = "";
}

function submitEntry() {
  if (canInterrupt.value) {
    emit("interrupt");
    return;
  }
  send();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    send();
  }
}

function modifyPlanApproval() {
  if (!hasPendingInputText.value) return;
  submitAskFreeform();
}

function queueResize() {
  if (resizeFrameId !== null) return;
  resizeFrameId = window.requestAnimationFrame(() => {
    resizeFrameId = null;
    resize();
  });
}

function measureInputScrollHeight(el: HTMLTextAreaElement): number {
  const measure = textareaMeasure.value;
  if (!measure) return el.scrollHeight;
  measure.value = el.value || " ";
  measure.style.width = `${el.clientWidth || el.getBoundingClientRect().width}px`;

  return measure.scrollHeight;
}

function resize() {
  const el = textarea.value;
  if (!el) return;
  const currentHeight =
    el.getBoundingClientRect().height ||
    Number.parseFloat(el.style.height) ||
    COMPOSER_INPUT_MIN_HEIGHT;
  const scrollHeight = measureInputScrollHeight(el);
  const nextHeight = Math.min(
    Math.max(scrollHeight, COMPOSER_INPUT_MIN_HEIGHT),
    COMPOSER_INPUT_MAX_HEIGHT,
  );
  el.style.height = `${currentHeight}px`;
  void el.offsetHeight;
  el.style.height = `${nextHeight}px`;

  if (overflowTimerId !== null) window.clearTimeout(overflowTimerId);
  overflowTimerId = null;
  el.style.overflowY = "hidden";
  el.scrollTop = 0;
  if (scrollHeight > COMPOSER_INPUT_MAX_HEIGHT) {
    overflowTimerId = window.setTimeout(() => {
      if (textarea.value !== el) return;
      el.style.overflowY = "auto";
      el.scrollTop = scrollHeight;
      overflowTimerId = null;
    }, COMPOSER_INPUT_TRANSITION_MS);
  }
}

function onInlineKeydown(e: KeyboardEvent) {
  const q = askQuestion.value;
  if (!q) return;
  if (e.key === "Escape" && askDismissable.value) {
    e.preventDefault();
    cancelAsk();
    return;
  }
  if (e.target instanceof HTMLTextAreaElement) return;
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    submitAsk();
    return;
  }
  if (q.mode === "confirm") return;
  const list = askOptionsWithId.value;
  const allIds = list.map((o) => o.id);
  if (allIds.length === 0) return;
  const cur = singleFocus.value ?? singlePick.value ?? allIds[0];
  const i = allIds.indexOf(cur);
  if (e.key === "ArrowDown") {
    e.preventDefault();
    highlightOption(allIds[(i + 1) % allIds.length]);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    highlightOption(allIds[(i - 1 + allIds.length) % allIds.length]);
  } else if (e.key === " " && q.mode === "single") {
    e.preventDefault();
    selectSingleOption(cur);
  } else if (e.key === " " && q.mode === "multi") {
    e.preventDefault();
    toggleMulti(cur);
  }
}

function decideToolConsent(decision: ToolConsentDecision, explicitMessage?: string) {
  const c = activeToolConsent.value;
  if (!c || toolSubmitting.value) return;
  toolSubmitting.value = decision;
  const message = decision === "deny"
    ? explicitMessage?.trim() || pendingInputText.value || "用户拒绝了此次工具调用"
    : undefined;
  emit("resolve-tool-consent", decision, message);
  if (decision === "deny") pendingText.value = "";
}

watch(inputValue, () => {
  void nextTick(queueResize);
});

watch(pendingKey, () => {
  if (!activeAsk.value) pendingText.value = "";
  toolExpanded.value = false;
  toolSubmitting.value = null;
  void nextTick(queueResize);
}, { immediate: true });

watch(
  () => askQuestion.value?.id,
  () => {
    void nextTick(queueResize);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (resizeFrameId !== null) {
    window.cancelAnimationFrame(resizeFrameId);
    resizeFrameId = null;
  }
  if (overflowTimerId !== null) {
    window.clearTimeout(overflowTimerId);
    overflowTimerId = null;
  }
});
</script>

<template>
  <div class="chat-composer">
    <Transition name="chat-composer-pending-panel">
      <div
        v-if="hasPendingPanel"
        :key="pendingKey"
        class="chat-composer__pending-panel"
      >
        <div class="chat-composer__pending-panel-inner">
          <section
            v-if="activeAsk && askQuestion"
            class="composer-inline composer-inline--ask"
            :class="{
              'composer-inline--danger': askQuestion.danger,
              'composer-inline--plan': askIsPlanApproval,
            }"
            role="region"
            aria-live="assertive"
            :aria-label="askTitle"
            tabindex="-1"
            @keydown="onInlineKeydown"
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
                <X :size="14" />
              </button>
            </header>

            <div v-if="!askIsPlanApproval" class="composer-inline__body">
              <div class="composer-inline__question">
                <span
                  v-if="askQuestion.header"
                  class="composer-inline__chip"
                >{{ askQuestion.header }}</span>
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
                      'is-active': activeAskOptionId === opt.id,
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
                        <Check
                          v-if="askQuestion.mode === 'multi' && multiPicks.has(opt.id)"
                          :size="12"
                        />
                      </span>
                      <span class="composer-inline__option-main">
                        <span class="composer-inline__option-label">
                          {{ opt.label }}
                          <span v-if="opt.recommended" class="composer-inline__badge">推荐</span>
                        </span>
                        <span
                          v-if="opt.description"
                          class="composer-inline__option-desc"
                        >{{ opt.description }}</span>
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
              v-if="askQuestion.mode === 'confirm' && !askIsPlanApproval"
              class="composer-inline__actions"
            >
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

              <button type="button" class="ghost composer-inline__btn" @click="confirmAskNo">
                {{ askQuestion.cancelLabel ?? "不要" }}
              </button>
              <button
                type="button"
                class="composer-inline__btn"
                :class="askQuestion.danger ? 'ghost danger' : 'primary'"
                @click="submitAsk"
              >
                {{ askQuestion.confirmLabel ?? "好的" }}
              </button>
            </footer>
          </section>

          <section
            v-else-if="activeToolConsent"
            class="composer-inline composer-inline--tool"
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
                  <span class="composer-inline__tool-name">{{ activeToolConsent.toolName }}</span>
                  <span class="composer-inline__headline">{{ toolHeadline }}</span>
                </div>
                <p v-if="toolInlinePreview" class="composer-inline__preview-line">{{ toolInlinePreview }}</p>
                <p v-if="toolSubtitle" class="composer-inline__subtitle">{{ toolSubtitle }}</p>
              </div>

              <button
                v-if="toolInputJson && toolInputJson !== '{}'"
                type="button"
                class="composer-inline__toggle"
                :aria-expanded="toolExpanded"
                @click="toolExpanded = !toolExpanded"
              >
                <component
                  :is="toolExpanded ? ChevronDown : ChevronRight"
                  :size="12"
                  aria-hidden="true"
                />
                {{ toolExpanded ? "收起" : "查看入参" }}
              </button>
            </div>

            <pre v-if="toolExpanded" class="composer-inline__details">{{ toolInputJson }}</pre>
          </section>
        </div>
      </div>
    </Transition>

    <div
      class="chat-composer__entry-row"
      :class="{ 'chat-composer__entry-row--pending': hasPending }"
    >
      <textarea
        v-if="!hasPending || !askUsesInputActions || askOtherSelected"
        ref="textareaMeasure"
        class="chat-composer__input chat-composer__input-measure"
        rows="1"
        tabindex="-1"
        aria-hidden="true"
      />
      <textarea
        v-if="!hasPending || !askUsesInputActions || askOtherSelected"
        ref="textarea"
        v-model="inputValue"
        class="chat-composer__input"
        rows="1"
        :placeholder="inputPlaceholder"
        @keydown="onKeydown"
      />

      <Transition name="chat-composer-entry-actions" mode="out-in">
        <div
          v-if="hasPending"
          :key="pendingEntryActionsKey"
          class="chat-composer__entry-actions"
        >
          <button
            v-if="askUsesInputActions && askQuestion?.skippable !== false && askTotal > 1"
            type="button"
            class="ghost composer-inline__skip composer-inline__btn"
            @click="skipAsk"
          >
            跳过
          </button>

          <div v-if="askUsesInputActions" class="chat-composer__pending-actions">
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
              type="button"
              class="primary composer-inline__btn"
              :disabled="!canAskSubmit"
              @click="submitAsk"
            >
              {{ askIsLast ? "完成" : "继续" }}
              <ArrowRight v-if="!askIsLast" :size="13" aria-hidden="true" />
            </button>
          </div>

          <div v-else-if="askIsPlanApproval" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ghost composer-inline__btn"
              :disabled="!hasPendingInputText"
              @click="modifyPlanApproval"
            >
              {{ hasPendingInputText ? "修改" : "忽略" }}
            </button>
            <button
              type="button"
              class="primary composer-inline__btn"
              @click="submitAsk"
            >
              同意
            </button>
          </div>

          <div v-else-if="activeToolConsent" class="chat-composer__pending-actions">
            <button
              type="button"
              class="ghost composer-inline__btn"
              :disabled="toolSubmitting !== null || !hasPendingInputText"
              @click="decideToolConsent('deny')"
            >
              {{ toolSubmitting === "deny" ? "处理中..." : hasPendingInputText ? "修改" : "忽略" }}
            </button>
            <button
              type="button"
              class="composer-inline__btn"
              :class="toolDanger ? 'ghost danger' : 'primary'"
              :disabled="toolSubmitting !== null"
              @click="decideToolConsent('allow')"
            >
              {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
            </button>
          </div>

          <button
            v-if="!hasPending || (!askUsesInputActions && !askIsPlanApproval && !activeToolConsent)"
            type="button"
            class="chat-composer__send"
            :class="{ 'chat-composer__send--interrupt': canInterrupt }"
            :disabled="!canSubmitEntry"
            :title="sendTitle"
            :aria-label="sendAriaLabel"
            @click="submitEntry"
          >
            <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
          </button>
        </div>
      </Transition>
    </div>

    <Transition name="chat-composer-stack">
      <div
        v-if="!hasPending && attachments?.length"
        class="chat-composer__attachments"
        aria-label="待发送附件"
      >
        <span
          v-for="attachment in attachments"
          :key="attachment.id"
          class="chat-attachment-chip"
          :title="attachment.path"
        >
          <Paperclip :size="13" aria-hidden="true" />
          <span class="chat-attachment-chip__name">{{ attachment.name }}</span>
          <button
            type="button"
            class="chat-attachment-chip__remove"
            :aria-label="`移除附件 ${attachment.name}`"
            @click="emit('remove-attachment', attachment.id)"
          >
            <X :size="12" aria-hidden="true" />
          </button>
        </span>
      </div>
    </Transition>

    <Transition name="chat-composer-stack">
      <div v-if="!hasPending" class="chat-composer__row">
        <div class="chat-composer__group">
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            title="添加附件"
            aria-label="添加附件"
            @click="emit('pick-attachments')"
          >
            <Paperclip :size="14" aria-hidden="true" />
          </button>
          <Dropdown
            :model-value="state.permission"
            :options="permissionOptions"
            :icon="ShieldCheck"
            @update:model-value="setPermission"
          />
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': state.planMode }"
            :title="state.planMode ? '本轮先制定计划' : '直接执行'"
            :aria-label="state.planMode ? '关闭计划模式' : '开启计划模式'"
            :aria-pressed="state.planMode"
            @click="togglePlanMode"
          >
            <ListChecks :size="14" aria-hidden="true" />
          </button>
        </div>

        <button
          type="button"
          class="chat-composer__send"
          :class="{ 'chat-composer__send--interrupt': canInterrupt }"
          :disabled="!canSubmitEntry"
          :title="sendTitle"
          :aria-label="sendAriaLabel"
          @click="submitEntry"
        >
          <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
        </button>
      </div>
    </Transition>
  </div>
</template>
