<script setup lang="ts">
import { ref } from "vue";
import { Plus } from "lucide-vue-next";
import type {
  AgentTimelineEvent,
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
  ChatContextUsage,
  ChatSlashCommandWorkflow,
  LiliaThreadGoal,
  LiliaReviewTarget,
  Project,
  SuggestionItem,
} from "@lilia/contracts";
import ChatTranscript from "../../components/chat/ChatTranscript.vue";
import ChatComposer from "../../components/chat/ChatComposer.vue";
import ComposerProjectPicker from "../../components/chat/ComposerProjectPicker.vue";
import ChatSidebarHost from "../../components/chat/ChatSidebarHost.vue";
import ImageViewer from "../../components/chat/ImageViewer.vue";
import type { LiliaBatchApplyInput } from "../../components/chat/liliaBatchApply";
import type { ChatImageViewerSource } from "../../components/chat/imageViewer";
import TodoFloat from "../../components/todo/TodoFloat.vue";
import type { PendingAsk } from "../../composables/useAskUser";
import type {
  PendingAgentAction,
  PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import type {
  ToolConsentDecision,
  ToolConsentRequest,
  ToolConsentUpdatedInput,
} from "../../services/chat";
import type { TaskTodo } from "../../services/todos";

defineProps<{
  taskId: string;
  projectId?: string;
  isPopup: boolean;
  shouldRenderChat: boolean;
  isPopupPending: boolean;
  shouldShowContextLoading: boolean;
  isContextLoading: boolean;
  fileDropActive: boolean;
  timelineEvents: AgentTimelineEvent[];
  emptyHeadline: string;
  isTurnRunning: boolean;
  projectCwd: string | null;
  contextSearchCwd: string | null;
  activePlanApprovalTurnId: string | null;
  userSendScrollKey: number;
  restoreDraftKey: number;
  restoreDraftContent: string;
  insertDraftTextKey: number;
  insertDraftTextContent: string;
  pendingAgentActions: PendingAgentAction[];
  hasBlockingPendingAction: boolean;
  currentLiliaGoal: LiliaThreadGoal | null;
  showExpiredPendingActions: boolean;
  canRetryEvent: (event: AgentTimelineEvent) => boolean;
  composerState: ChatComposerState;
  contextUsage: ChatContextUsage | null;
  attachments: ChatAttachment[];
  appendAttachmentsToEndKey: number;
  pendingAsk: PendingAsk | null;
  toolConsent: ToolConsentRequest | null;
  viewingImage: ChatImageViewerSource | null;
  suggestions: SuggestionItem[];
  suggestionsStatus: "idle" | "loading" | "empty" | "error";
  suggestionsVisible: boolean;
  showDraftProjectPicker?: boolean;
  draftProjectPickerProjects?: Project[];
}>();

const emit = defineEmits<{
  "resolve-pending-agent-action": [resolution: PendingAgentActionResolution];
  "retry-event": [event: AgentTimelineEvent];
  "open-image": [image: ChatImageViewerSource];
  "close-image": [];
  "insert-guide": [todo: TaskTodo];
  "set-lilia-goal": [objective: string];
  "refresh-lilia-goal": [];
  "clear-lilia-goal": [];
  "insert-draft-text": [text: string];
  send: [content: string, attachments: ChatAttachment[]];
  "start-lilia-review": [
    content: string,
    attachments: ChatAttachment[],
    target: LiliaReviewTarget,
  ];
  "start-lilia-fix-suggestion": [
    content: string,
    attachments: ChatAttachment[],
    target: LiliaReviewTarget,
  ];
  "start-lilia-compact": [];
  "start-session-fork": [];
  "open-lilia-iab": [];
  "execute-slash-command": [workflow: ChatSlashCommandWorkflow];
  "start-lilia-batch-apply": [input: LiliaBatchApplyInput];
  interrupt: [];
  "update-composer": [next: ChatComposerState];
  "remove-attachment": [attachmentId: string];
  "pick-attachments": [];
  "add-context-attachment": [attachment: ChatAttachment];
  "resolve-ask-user": [result: AskUserResult];
  "resolve-tool-consent": [
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ];
  "refresh-suggestions": [];
  "select-draft-project": [projectId: string];
  "created-draft-project": [project: Project];
  "draft-project-picker-error": [message: string];
}>();

const chatPageRef = ref<HTMLElement | null>(null);
const composerRef = ref<InstanceType<typeof ChatComposer> | null>(null);

function focusComposer() {
  composerRef.value?.focusInput();
}

function getComposerDraftSnapshot() {
  return composerRef.value?.getDraftSnapshot() ?? { content: "" };
}

defineExpose({ chatPageRef, focusComposer, getComposerDraftSnapshot });

function emitSend(content: string, outgoingAttachments: ChatAttachment[]) {
  emit("send", content, outgoingAttachments);
}
</script>

<template>
  <section
    v-if="shouldRenderChat"
    ref="chatPageRef"
    class="chat-page"
    :class="{ 'chat-page--popup': isPopup }"
  >
    <div class="chat">
      <div class="chat-layout">
        <div class="chat-layout__main">
          <div
            v-if="fileDropActive"
            class="chat-file-drop-overlay"
            aria-live="polite"
            role="status"
          >
            <Plus class="chat-file-drop-overlay__icon" aria-hidden="true" />
            <span class="chat-file-drop-overlay__text">拖入文件以追加到输入框</span>
          </div>
          <ChatTranscript
            :timeline-events="timelineEvents"
            :empty-headline="emptyHeadline"
            :project-id="projectId"
            :is-thinking="isTurnRunning"
            :project-cwd="projectCwd"
            :active-plan-approval-turn-id="activePlanApprovalTurnId"
            :force-scroll-bottom-key="userSendScrollKey"
            :pending-agent-actions="pendingAgentActions"
            :show-expired-pending-actions="showExpiredPendingActions"
            :can-retry-event="canRetryEvent"
            :can-start-lilia-batch-apply="!isTurnRunning && !hasBlockingPendingAction"
            :can-start-session-fork="!isTurnRunning && !hasBlockingPendingAction"
            @resolve-pending-agent-action="emit('resolve-pending-agent-action', $event)"
            @retry-event="emit('retry-event', $event)"
            @open-image="emit('open-image', $event)"
            @insert-draft-text="emit('insert-draft-text', $event)"
            @start-lilia-batch-apply="emit('start-lilia-batch-apply', $event)"
            @start-session-fork="emit('start-session-fork')"
          >
            <template #controls>
              <div class="chat-controls">
                <TodoFloat
                  v-if="taskId"
                  :task-id="taskId"
                  :show-goal="true"
                  :goal="currentLiliaGoal"
                  :goal-disabled="isTurnRunning || pendingAgentActions.some((action) => action.kind !== 'title_update')"
                  @insert-guide="emit('insert-guide', $event)"
                  @set-lilia-goal="emit('set-lilia-goal', $event)"
                  @refresh-lilia-goal="emit('refresh-lilia-goal')"
                  @clear-lilia-goal="emit('clear-lilia-goal')"
                />
                <ChatComposer
                  ref="composerRef"
                  :state="composerState"
                  :attachments="attachments"
                  :append-attachments-to-end-key="appendAttachmentsToEndKey"
                  :project-cwd="contextSearchCwd"
                  :sending="isTurnRunning"
                  :compact-disabled="hasBlockingPendingAction"
                  :context-usage="contextUsage"
                  :pending-ask="pendingAsk"
                  :tool-consent="toolConsent"
                  :suggestions="suggestions"
                  :suggestions-status="suggestionsStatus"
                  :suggestions-visible="suggestionsVisible"
                  :restore-draft-key="restoreDraftKey"
                  :restore-draft-content="restoreDraftContent"
                  :insert-draft-text-key="insertDraftTextKey"
                  :insert-draft-text-content="insertDraftTextContent"
                  @send="emitSend"
                  @start-lilia-review="(content, outgoingAttachments, target) => emit('start-lilia-review', content, outgoingAttachments, target)"
                  @start-lilia-fix-suggestion="(content, outgoingAttachments, target) => emit('start-lilia-fix-suggestion', content, outgoingAttachments, target)"
                  @start-lilia-compact="emit('start-lilia-compact')"
                  @open-lilia-iab="emit('open-lilia-iab')"
                  @execute-slash-command="emit('execute-slash-command', $event)"
                  @interrupt="emit('interrupt')"
                  @update:state="emit('update-composer', $event)"
                  @remove-attachment="emit('remove-attachment', $event)"
                  @pick-attachments="emit('pick-attachments')"
                  @add-context-attachment="emit('add-context-attachment', $event)"
                  @resolve-ask-user="emit('resolve-ask-user', $event)"
                  @resolve-tool-consent="(decision, message, updatedInput) => emit('resolve-tool-consent', decision, message, updatedInput)"
                  @open-image="emit('open-image', $event)"
                  @refresh-suggestions="emit('refresh-suggestions')"
                />
                <ComposerProjectPicker
                  v-if="showDraftProjectPicker"
                  :projects="draftProjectPickerProjects ?? []"
                  @select-project="emit('select-draft-project', $event)"
                  @created-project="emit('created-draft-project', $event)"
                  @error="emit('draft-project-picker-error', $event)"
                />
              </div>
            </template>
          </ChatTranscript>
        </div>
        <ChatSidebarHost
          v-if="!isPopup"
          :task-id="taskId"
          :project-id="projectId"
          :project-cwd="projectCwd"
        />
      </div>
    </div>
    <ImageViewer
      v-if="viewingImage"
      :image="viewingImage"
      @close="emit('close-image')"
    />
  </section>

  <section
    v-else-if="isPopupPending"
    class="chat-page chat-page--popup chat-page--pending"
    aria-busy="true"
  >
    <div class="chat">
      <div class="chat-layout">
        <div class="chat-layout__main">
          <div
            v-if="shouldShowContextLoading"
            class="popup-context-loading"
            role="status"
          >
            正在加载对话…
          </div>
        </div>
      </div>
    </div>
  </section>

  <section v-else-if="isContextLoading">
    <div class="empty-state">正在加载对话…</div>
  </section>

  <section v-else>
    <div class="empty-state">未找到任务 <code>{{ taskId }}</code></div>
  </section>
</template>
