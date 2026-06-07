<script setup lang="ts">
import { ArrowLeft, ArrowRight, ArrowUp, Square } from "lucide-vue-next";
import type { ToolConsentDecision } from "../../services/chat";

defineProps<{
  askUsesInputActions: boolean;
  askQuestionSkippable: boolean;
  askTotal: number;
  canGoPrev: boolean;
  canAskSubmit: boolean;
  askIsLast: boolean;
  askIsPlanApproval: boolean;
  hasPendingInputText: boolean;
  hasToolConsent: boolean;
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
    <button
      v-if="askUsesInputActions && askQuestionSkippable && askTotal > 1"
      type="button"
      class="ghost composer-inline__skip composer-inline__btn"
      :disabled="actionsBlocked"
      @click="emit('skipAsk')"
    >
      跳过
    </button>

    <div v-if="askUsesInputActions" class="chat-composer__pending-actions">
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
        class="primary composer-inline__btn"
        :disabled="actionsBlocked || !canAskSubmit"
        @click="emit('submitAsk')"
      >
        {{ askIsLast ? "完成" : "继续" }}
        <ArrowRight v-if="!askIsLast" :size="13" aria-hidden="true" />
      </button>
    </div>

    <div v-else-if="askIsPlanApproval" class="chat-composer__pending-actions">
      <button
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked || !hasPendingInputText"
        @click="emit('modifyPlanApproval')"
      >
        {{ hasPendingInputText ? "修改" : "忽略" }}
      </button>
      <button
        type="button"
        class="primary composer-inline__btn"
        :disabled="actionsBlocked"
        @click="emit('submitAsk')"
      >
        同意
      </button>
    </div>

    <div v-else-if="hasToolConsent" class="chat-composer__pending-actions">
      <button
        type="button"
        class="ghost composer-inline__btn"
        :disabled="actionsBlocked || toolSubmitting !== null || (!isEditingToolCommand && !hasPendingInputText)"
        @click="isEditingToolCommand ? emit('cancelToolCommandEdit') : emit('decideToolConsent', 'deny')"
      >
        {{ toolSubmitting === "deny" ? "处理中..." : isEditingToolCommand ? "取消" : hasPendingInputText ? "修改" : "忽略" }}
      </button>
      <button
        type="button"
        class="composer-inline__btn"
        :class="toolDanger ? 'ghost danger' : 'primary'"
        :disabled="actionsBlocked || toolSubmitting !== null || toolCommandIsEmpty"
        @click="emit('decideToolConsent', 'allow')"
      >
        {{ toolSubmitting === "allow" ? "处理中..." : toolDanger ? "同意执行" : "同意" }}
      </button>
    </div>

    <button
      v-if="!askUsesInputActions && !askIsPlanApproval && !hasToolConsent"
      type="button"
      class="chat-composer__send"
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
