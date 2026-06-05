<script setup lang="ts">
import { ref } from "vue";
import { Plus } from "lucide-vue-next";
import type {
  AgentTimelineEvent,
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
  SuggestionItem,
} from "@lilia/contracts";
import ChatTranscript from "../../components/chat/ChatTranscript.vue";
import ChatComposer from "../../components/chat/ChatComposer.vue";
import ChatSidebarHost from "../../components/chat/ChatSidebarHost.vue";
import ImageViewer from "../../components/chat/ImageViewer.vue";
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
  pendingAgentActions: PendingAgentAction[];
  showExpiredPendingActions: boolean;
  canRetryEvent: (event: AgentTimelineEvent) => boolean;
  composerState: ChatComposerState;
  attachments: ChatAttachment[];
  appendAttachmentsToEndKey: number;
  pendingAsk: PendingAsk | null;
  toolConsent: ToolConsentRequest | null;
  viewingImage: ChatImageViewerSource | null;
  suggestions: SuggestionItem[];
  suggestionsLoading: boolean;
  suggestionsVisible: boolean;
}>();

const emit = defineEmits<{
  "resolve-pending-agent-action": [resolution: PendingAgentActionResolution];
  "retry-event": [event: AgentTimelineEvent];
  "open-image": [image: ChatImageViewerSource];
  "close-image": [];
  "insert-guide": [todo: TaskTodo];
  send: [content: string, attachments: ChatAttachment[]];
  interrupt: [];
  "update-composer": [next: ChatComposerState];
  "remove-attachment": [attachmentId: string];
  "pick-attachments": [];
  "add-context-attachment": [attachment: ChatAttachment];
  "refresh-suggestions": [];
  "resolve-ask-user": [result: AskUserResult];
  "resolve-tool-consent": [
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ];
}>();

const chatPageRef = ref<HTMLElement | null>(null);

defineExpose({ chatPageRef });

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
            :is-thinking="isTurnRunning"
            :project-cwd="projectCwd"
            :active-plan-approval-turn-id="activePlanApprovalTurnId"
            :force-scroll-bottom-key="userSendScrollKey"
            :pending-agent-actions="pendingAgentActions"
            :show-expired-pending-actions="showExpiredPendingActions"
            :can-retry-event="canRetryEvent"
            @resolve-pending-agent-action="emit('resolve-pending-agent-action', $event)"
            @retry-event="emit('retry-event', $event)"
            @open-image="emit('open-image', $event)"
          >
            <template #controls>
              <div class="chat-controls">
                <TodoFloat
                  v-if="taskId"
                  :task-id="taskId"
                  @insert-guide="emit('insert-guide', $event)"
                />
                <ChatComposer
                  :state="composerState"
                  :attachments="attachments"
                  :append-attachments-to-end-key="appendAttachmentsToEndKey"
                  :project-cwd="contextSearchCwd"
                  :sending="isTurnRunning"
                  :pending-ask="pendingAsk"
                  :tool-consent="toolConsent"
                  :suggestions="suggestions"
                  :suggestions-loading="suggestionsLoading"
                  :suggestions-visible="suggestionsVisible"
                  @send="emitSend"
                  @interrupt="emit('interrupt')"
                  @update:state="emit('update-composer', $event)"
                  @remove-attachment="emit('remove-attachment', $event)"
                  @pick-attachments="emit('pick-attachments')"
                  @add-context-attachment="emit('add-context-attachment', $event)"
                  @refresh-suggestions="emit('refresh-suggestions')"
                  @resolve-ask-user="emit('resolve-ask-user', $event)"
                  @resolve-tool-consent="(decision, message, updatedInput) => emit('resolve-tool-consent', decision, message, updatedInput)"
                  @open-image="emit('open-image', $event)"
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
