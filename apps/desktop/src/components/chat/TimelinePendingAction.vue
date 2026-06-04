<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { AskUserResult } from "@lilia/contracts";
import { useAskUserInteraction } from "../../composables/useAskUserInteraction";
import { useEditableToolCommand } from "../../composables/useEditableToolCommand";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import { useToolConsentPresentation } from "../../composables/useToolConsentPresentation";
import type { ToolConsentDecision } from "../../services/chat";
import AskUserInlinePrompt from "./AskUserInlinePrompt.vue";
import ToolConsentInlinePrompt from "./ToolConsentInlinePrompt.vue";

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
  askHasPreview,
  askFocusedOption,
  askOtherSelected,
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
const {
  commandDraft: toolCommandDraft,
  isEditingCommand: isEditingToolCommand,
  hasEditableCommand,
  commandIsEmpty: toolCommandIsEmpty,
  updatedCommandInput,
  beginCommandEdit,
  cancelCommandEdit,
} = useEditableToolCommand(toolRequest);
const hasFreeformText = computed(() => freeformText.value.trim().length > 0);
const hasToolMessage = computed(() => toolMessage.value.trim().length > 0);

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
  if (decision === "allow" && toolCommandIsEmpty.value) return;
  toolSubmitting.value = decision;
  const updatedInput = decision === "allow" ? updatedCommandInput.value : undefined;
  emit("resolve", {
    kind: "tool_consent",
    requestId: props.action.requestId,
    decision,
    message: decision === "deny"
      ? toolMessage.value.trim() || "用户拒绝了此次工具调用"
      : undefined,
    ...(updatedInput ? { updatedInput } : {}),
  });
}

watch(actionKey, () => {
  toolExpanded.value = false;
  toolMessage.value = "";
  toolSubmitting.value = null;
}, { immediate: true });
</script>

<template>
  <ToolConsentInlinePrompt
    v-if="props.action.kind === 'tool_consent' && toolRequest"
    root-class="timeline-pending-action composer-inline composer-inline--tool"
    :active-tool-consent="toolRequest"
    :tool-danger="toolDanger"
    :tool-icon="toolIcon"
    :tool-headline="toolHeadline"
    :tool-input-json="toolInputJson"
    :tool-subtitle="toolSubtitle"
    :tool-expanded="toolExpanded"
    :is-editing-tool-command="isEditingToolCommand"
    :has-editable-command="hasEditableCommand"
    :tool-command-draft="toolCommandDraft"
    @update-tool-expanded="toolExpanded = $event"
    @update-tool-command-draft="toolCommandDraft = $event"
    @begin-command-edit="beginCommandEdit"
  >
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
          :disabled="toolSubmitting !== null || (!isEditingToolCommand && !hasToolMessage)"
          @click="isEditingToolCommand ? cancelCommandEdit() : decideTool('deny')"
        >
          {{ toolSubmitting === "deny" ? "处理中..." : isEditingToolCommand ? "取消" : hasToolMessage ? "修改" : "忽略" }}
        </button>
        <button
          type="button"
          class="primary composer-inline__btn"
          :disabled="toolSubmitting !== null || toolCommandIsEmpty"
          @click="decideTool('allow')"
        >
          {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
        </button>
      </div>
    </div>
  </ToolConsentInlinePrompt>

  <section
    v-else-if="props.action.kind === 'plan_approval'"
    class="timeline-pending-action timeline-pending-action--plan"
    role="region"
    :aria-label="askTitle"
  >
    <textarea
      v-model="freeformText"
      class="timeline-pending-action__input"
      rows="1"
      placeholder="修改要求"
    />
    <div class="composer-inline__actions">
      <button
        type="button"
        class="ghost composer-inline__btn"
        :disabled="!hasFreeformText"
        @click="submitAskFreeform()"
      >
        {{ hasFreeformText ? "修改" : "忽略" }}
      </button>
      <button type="button" class="primary composer-inline__btn" @click="submitAsk">
        同意
      </button>
    </div>
  </section>

  <AskUserInlinePrompt
    v-else-if="activeAsk && askQuestion"
    v-model:freeform-text="freeformText"
    root-class="timeline-pending-action composer-inline composer-inline--ask"
    input-class="timeline-pending-action__input composer-inline__other-input"
    :active-ask="activeAsk"
    :ask-question="askQuestion"
    :ask-title="askTitle"
    :ask-index="askIndex"
    :ask-total="askTotal"
    :ask-dismissable="askDismissable"
    :ask-is-last="askIsLast"
    :ask-options-with-id="askOptionsWithId"
    :ask-has-preview="askHasPreview"
    :ask-focused-option="askFocusedOption"
    :active-option-id="activeOptionId"
    :single-pick="singlePick"
    :multi-picks="multiPicks"
    :can-go-prev="canGoPrev"
    :can-ask-submit="canAskSubmit"
    :ask-other-selected="askOtherSelected"
    show-choice-footer
    @cancel-ask="cancelAsk"
    @highlight-option="highlightOption"
    @clear-option-highlight="clearOptionHighlight"
    @focus-option="focusOption"
    @select-single-option="selectSingleOption"
    @toggle-multi="toggleMulti"
    @skip-ask="skipAsk"
    @back-ask="backAsk"
    @confirm-ask-no="confirmAskNo"
    @submit-ask="submitAsk"
  />
</template>
