<script setup lang="ts">
import type { Component } from "vue";
import type { AskUserOption, AskUserQuestion } from "@lilia/contracts";
import type { PendingAsk } from "../../composables/useAskUser";
import type { ToolConsentRequest } from "../../services/chat";
import AskUserInlinePrompt from "./AskUserInlinePrompt.vue";
import ToolConsentInlinePrompt from "./ToolConsentInlinePrompt.vue";

type AskOptionView = AskUserOption & { id: string };

defineProps<{
  activeAsk: PendingAsk | null;
  askQuestion: AskUserQuestion | null | undefined;
  askTitle: string;
  askIndex: number;
  askTotal: number;
  askDismissable: boolean;
  askIsPlanApproval: boolean;
  askOptionsWithId: AskOptionView[];
  askHasPreview: boolean;
  askFocusedOption: AskOptionView | null | undefined;
  activeAskOptionId: string | null | undefined;
  singlePick: string | null | undefined;
  multiPicks: Set<string>;
  canGoPrev: boolean;
  activeToolConsent: ToolConsentRequest | null;
  toolDanger: boolean;
  toolIcon: Component;
  toolHeadline: string;
  toolInlinePreview: string | null;
  toolInputJson: string | null;
  toolSubtitle: string | null;
  toolExpanded: boolean;
  isEditingToolCommand: boolean;
  hasEditableCommand: boolean;
  toolCommandDraft: string;
  actionsBlocked: boolean;
}>();

const emit = defineEmits<{
  keydown: [event: KeyboardEvent];
  cancelAsk: [];
  highlightOption: [id: string];
  clearOptionHighlight: [id: string];
  focusOption: [id: string];
  selectSingleOption: [id: string];
  toggleMulti: [id: string];
  skipAsk: [];
  backAsk: [];
  confirmAskNo: [];
  submitAsk: [];
  updateToolExpanded: [expanded: boolean];
  updateToolCommandDraft: [draft: string];
  beginCommandEdit: [];
}>();
</script>

<template>
  <div class="chat-composer__pending-panel">
    <div class="chat-composer__pending-panel-inner">
      <AskUserInlinePrompt
        v-if="activeAsk && askQuestion"
        :active-ask="activeAsk"
        :ask-question="askQuestion"
        :ask-title="askTitle"
        :ask-index="askIndex"
        :ask-total="askTotal"
        :ask-dismissable="askDismissable"
        :ask-is-last="false"
        :ask-is-plan-approval="askIsPlanApproval"
        :ask-options-with-id="askOptionsWithId"
        :ask-has-preview="askHasPreview"
        :ask-focused-option="askFocusedOption"
        :active-option-id="activeAskOptionId"
        :single-pick="singlePick"
        :multi-picks="multiPicks"
        :can-go-prev="canGoPrev"
        :can-ask-submit="false"
        :actions-blocked="actionsBlocked"
        :show-confirm-footer="true"
        tabindex="-1"
        @keydown="emit('keydown', $event)"
        @cancel-ask="emit('cancelAsk')"
        @highlight-option="emit('highlightOption', $event)"
        @clear-option-highlight="emit('clearOptionHighlight', $event)"
        @focus-option="emit('focusOption', $event)"
        @select-single-option="emit('selectSingleOption', $event)"
        @toggle-multi="emit('toggleMulti', $event)"
        @skip-ask="emit('skipAsk')"
        @back-ask="emit('backAsk')"
        @confirm-ask-no="emit('confirmAskNo')"
        @submit-ask="emit('submitAsk')"
      />

      <ToolConsentInlinePrompt
        v-else-if="activeToolConsent"
        :active-tool-consent="activeToolConsent"
        :tool-danger="toolDanger"
        :tool-icon="toolIcon"
        :tool-headline="toolHeadline"
        :tool-inline-preview="toolInlinePreview"
        :tool-input-json="toolInputJson"
        :tool-subtitle="toolSubtitle"
        :tool-expanded="toolExpanded"
        :is-editing-tool-command="isEditingToolCommand"
        :has-editable-command="hasEditableCommand"
        :tool-command-draft="toolCommandDraft"
        show-inline-preview
        @update-tool-expanded="emit('updateToolExpanded', $event)"
        @update-tool-command-draft="emit('updateToolCommandDraft', $event)"
        @begin-command-edit="emit('beginCommandEdit')"
      />
    </div>
  </div>
</template>
