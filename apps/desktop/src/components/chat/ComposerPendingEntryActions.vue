<script setup lang="ts">
import { ArrowLeft, ArrowRight, ArrowUp, Check, Pencil, SkipForward, Square, X } from "@lucide/vue";
import type { ToolConsentDecision } from "../../services/chat";
import type { ComposerPendingEntryActionsMode } from "./useComposerPendingInteraction";

defineProps<{
  mode: ComposerPendingEntryActionsMode;
  askQuestionSkippable: boolean;
  askTotal: number;
  canGoPrev: boolean;
  canAskSubmit: boolean;
  askIsLast: boolean;
  autoDecisionText: string;
  hasPendingInputText: boolean;
  toolSubmitting: ToolConsentDecision | null;
  isEditingToolCommand: boolean;
  toolDanger: boolean;
  toolCommandIsEmpty: boolean;
  canInterrupt: boolean;
  canSubmitEntry: boolean;
  actionsBlocked: boolean;
  sendTitle: string;
  sendAriaLabel: string;
}>();

const emit = defineEmits<{
  skipAsk: [];
  backAsk: [];
  submitAsk: [];
  modifyPlanApproval: [];
  cancelToolCommandEdit: [];
  decideToolConsent: [decision: ToolConsentDecision];
  submitEntry: [];
}>();
</script>

<template>
  <div class="chat-composer__entry-actions">
    <div
      v-if="autoDecisionText"
      class="composer-inline__auto-decision"
      role="status"
    >
      {{ autoDecisionText }}
    </div>

    <button
      v-if="mode === 'ask-input' && askQuestionSkippable && askTotal > 1"
      type="button"
      class="ui-button ui-button--ghost composer-inline__skip composer-inline__btn"
      data-agent-id="chat.pending.skip"
      :disabled="actionsBlocked"
      title="跳过"
      @click="emit('skipAsk')"
    >
      <SkipForward :size="13" aria-hidden="true" />
      <span class="composer-inline__btn-label">跳过</span>
    </button>

    <div v-if="mode === 'ask-input'" class="chat-composer__pending-actions">
      <button
        v-if="canGoPrev"
        type="button"
        class="ui-button ui-button--ghost composer-inline__btn"
        data-agent-id="chat.pending.back"
        :disabled="actionsBlocked"
        title="上一题"
        @click="emit('backAsk')"
      >
        <ArrowLeft :size="13" aria-hidden="true" />
        <span class="composer-inline__btn-label">上一题</span>
      </button>
      <button
        type="button"
        class="ui-button ui-button--primary composer-inline__btn"
        data-agent-id="chat.pending.continue"
        :disabled="actionsBlocked || !canAskSubmit"
        :title="askIsLast ? '完成' : '继续'"
        @click="emit('submitAsk')"
      >
        <span class="composer-inline__btn-label">{{ askIsLast ? "完成" : "继续" }}</span>
        <Check v-if="askIsLast" :size="13" aria-hidden="true" />
        <ArrowRight v-if="!askIsLast" :size="13" aria-hidden="true" />
      </button>
    </div>

    <div v-else-if="mode === 'ask-plan'" class="chat-composer__pending-actions">
      <button
        type="button"
        class="ui-button ui-button--ghost composer-inline__btn"
        data-agent-id="chat.pending.plan.modify"
        :disabled="actionsBlocked || !hasPendingInputText"
        :title="hasPendingInputText ? '修改' : '忽略'"
        @click="emit('modifyPlanApproval')"
      >
        <component :is="hasPendingInputText ? Pencil : SkipForward" :size="13" aria-hidden="true" />
        <span class="composer-inline__btn-label">{{ hasPendingInputText ? "修改" : "忽略" }}</span>
      </button>
      <button
        type="button"
        class="ui-button ui-button--primary composer-inline__btn"
        data-agent-id="chat.pending.plan.accept"
        :disabled="actionsBlocked"
        title="同意"
        @click="emit('submitAsk')"
      >
        <Check :size="13" aria-hidden="true" />
        <span class="composer-inline__btn-label">同意</span>
      </button>
    </div>

    <div v-else-if="mode === 'tool'" class="chat-composer__pending-actions">
      <button
        type="button"
        class="ui-button ui-button--ghost composer-inline__btn"
        data-agent-id="chat.pending.tool.secondary"
        :disabled="actionsBlocked || toolSubmitting !== null || (!isEditingToolCommand && !hasPendingInputText)"
        :title="isEditingToolCommand ? '取消' : hasPendingInputText ? '修改' : '忽略'"
        @click="isEditingToolCommand ? emit('cancelToolCommandEdit') : emit('decideToolConsent', 'deny')"
      >
        <component :is="isEditingToolCommand ? X : hasPendingInputText ? Pencil : SkipForward" :size="13" aria-hidden="true" />
        <span class="composer-inline__btn-label">
          {{ toolSubmitting === "deny" ? "处理中..." : isEditingToolCommand ? "取消" : hasPendingInputText ? "修改" : "忽略" }}
        </span>
      </button>
      <button
        type="button"
        class="composer-inline__btn"
        data-agent-id="chat.pending.tool.allow"
        :class="toolDanger ? 'ui-button ui-button--ghost ui-button--danger' : 'ui-button ui-button--primary'"
        :disabled="actionsBlocked || toolSubmitting !== null || toolCommandIsEmpty"
        :title="toolDanger ? '同意执行' : '同意'"
        @click="emit('decideToolConsent', 'allow')"
      >
        <Check :size="13" aria-hidden="true" />
        <span class="composer-inline__btn-label">
          {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
        </span>
      </button>
    </div>

    <button
      v-if="mode === 'ask-confirm' || mode === 'none'"
      type="button"
      class="chat-composer__send"
      data-agent-id="chat.composer.send"
      :class="{ 'chat-composer__send--interrupt': canInterrupt }"
      :disabled="actionsBlocked || !canSubmitEntry"
      :title="sendTitle"
      :aria-label="sendAriaLabel"
      @click="emit('submitEntry')"
    >
      <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
    </button>
  </div>
</template>

