<script setup lang="ts">
/**
 * Task 详情 = 聊天面板。此页只编排路由上下文、timeline、composer 与附件控制器；
 * 具体聊天布局由 TaskDetailChatSurface 负责。
 */

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ChatAttachment, SuggestionItem } from "@lilia/contracts";
import { registerDebugChatSidebarPanel } from "../composables/useDebugChatSidebarPanel";
import TaskDetailChatSurface from "./taskDetail/TaskDetailChatSurface.vue";
import { useTaskAttachments } from "./taskDetail/useTaskAttachments";
import { useTaskComposerController } from "./taskDetail/useTaskComposerController";
import {
  useTaskConversationContext,
  type TaskDetailRouteProps,
} from "./taskDetail/useTaskConversationContext";
import { useTaskTimeline } from "./taskDetail/useTaskTimeline";
import { getConversationSuggestions } from "../services/chat";

const props = withDefaults(defineProps<{
  projectId?: string;
  taskId: string;
  variant?: "main" | "popup";
}>(), {
  variant: "main",
});

const routeProps = props as TaskDetailRouteProps;
const chatSurfaceRef = ref<InstanceType<typeof TaskDetailChatSurface> | null>(null);
const chatPageRef = computed<HTMLElement | null>(() =>
  chatSurfaceRef.value?.chatPageRef ?? null,
);
const sharedAttachments = ref<ChatAttachment[]>([]);
const suggestions = ref<SuggestionItem[]>([]);
let suggestionsSeq = 0;

const conversation = useTaskConversationContext(routeProps);
const timeline = useTaskTimeline({
  taskId: () => props.taskId,
  backend: () => composerController.composerForView.value.backend,
});
const composerController = useTaskComposerController({
  props: routeProps,
  context: conversation,
  timeline,
  attachments: sharedAttachments,
});
const attachmentController = useTaskAttachments({
  chatPageRef,
  attachments: sharedAttachments,
  hasContext: () => conversation.hasContext.value,
  canAcceptInteractiveDrop: composerController.canAcceptInteractiveDrop,
});

const {
  isPopup,
  project,
  hasContext,
  conversationRouteState,
  isContextLoading,
  isPopupPending,
  shouldRenderChat,
  shouldShowContextLoading,
  emptyHeadline,
  contextSearchCwd,
} = conversation;
const {
  timelineEvents,
  canRetryEvent,
} = timeline;
const {
  attachments,
  viewingImage,
  droppedAttachmentAppendKey,
  fileDropActive,
} = attachmentController;
const {
  composerForView,
  isTurnRunning,
  userSendScrollKey,
  restoreDraftKey,
  restoreDraftContent,
  pendingAskUser,
  pendingToolConsent,
  pendingAgentActions,
  blockingPendingAgentActions,
  pendingPlanApproval,
  nonInterruptMode,
} = composerController;

const shouldLoadSuggestions = computed(() =>
  !!props.projectId &&
  shouldRenderChat.value &&
  conversationRouteState.value.isLiveDraft &&
  timelineEvents.value.length === 0 &&
  !isTurnRunning.value &&
  blockingPendingAgentActions.value.length === 0 &&
  !pendingAskUser.value &&
  !pendingToolConsent.value,
);

async function loadSuggestions(forceRefresh = false) {
  if (!shouldLoadSuggestions.value) {
    suggestions.value = [];
    return;
  }
  const seq = ++suggestionsSeq;
  try {
    const next = await getConversationSuggestions(props.projectId ?? null, forceRefresh);
    if (seq === suggestionsSeq) suggestions.value = next;
  } catch (err) {
    console.error("[conversation-suggestions] load failed", err);
    if (seq === suggestionsSeq) suggestions.value = [];
  }
}

const unlisteners: UnlistenFn[] = [];
let unregisterDebugPanel: (() => void) | null = null;

function syncDebugPanelRegistration() {
  if (!hasContext.value || !composerController.agentInteractionSettings.debug.value) {
    unregisterDebugPanel?.();
    unregisterDebugPanel = null;
    return;
  }
  if (unregisterDebugPanel) return;
  unregisterDebugPanel = registerDebugChatSidebarPanel();
}

onMounted(async () => {
  if (!props.projectId) await conversation.ensureOrphanCwd();
  unlisteners.push(await attachmentController.installDragDropListener());
  unlisteners.push(...await composerController.installRuntimeListeners());
  if (conversation.isPopup.value) {
    await composerController.loadAgentInteractionSettings();
  } else {
    await Promise.all([
      conversation.hydrateMainContext(),
      composerController.loadAll(),
      composerController.loadAgentInteractionSettings(),
    ]);
  }
});

onUnmounted(async () => {
  timeline.disposeTimeline();
  unregisterDebugPanel?.();
  unregisterDebugPanel = null;
  for (const unlisten of unlisteners) {
    try {
      await unlisten();
    } catch {
      // ignore
    }
  }
  unlisteners.length = 0;
});

watch(
  () => [props.variant, props.projectId, props.taskId, conversation.hasContext.value] as const,
  ([variant, _projectId, _taskId, ready]) => {
    if (variant !== "popup" || !ready) return;
    void composerController.loadAll();
  },
  { immediate: true },
);

watch(
  () => [props.projectId, props.taskId] as const,
  async () => {
    suggestionsSeq += 1;
    suggestions.value = [];
    conversation.prepareForRouteChange();
    composerController.resetForRouteChange();
    timeline.resetTimeline();
    attachmentController.resetAttachments();
    timeline.resubscribeDebugTimeline();
    if (!props.projectId) await conversation.ensureOrphanCwd();
    if (!conversation.isPopup.value) {
      await conversation.hydrateMainContext();
      await composerController.loadAll();
    }
  },
);

watch(
  () => [
    shouldLoadSuggestions.value,
    props.projectId ?? "",
    props.taskId,
    timelineEvents.value.length,
  ] as const,
  ([ready]) => {
    if (!ready) {
      suggestionsSeq += 1;
      suggestions.value = [];
      return;
    }
    void loadSuggestions(false);
  },
  { immediate: true },
);

watch(
  () => [hasContext.value, composerController.agentInteractionSettings.debug.value] as const,
  syncDebugPanelRegistration,
  { immediate: true },
);

watch(
  () => [
    composerController.pendingAskUsers.value.length,
    composerController.pendingToolConsents.value.length,
    composerController.pendingPlanApproval.value?.turnId ?? "",
  ] as const,
  ([askCount, consentCount, planTurn], [prevAskCount, prevConsentCount, prevPlanTurn]) => {
    if (
      askCount > prevAskCount ||
      consentCount > prevConsentCount ||
      (planTurn && planTurn !== prevPlanTurn)
    ) {
      void composerController.scheduleUserGuideInsertion();
    }
  },
);
</script>

<template>
  <TaskDetailChatSurface
    ref="chatSurfaceRef"
    :task-id="taskId"
    :project-id="projectId"
    :is-popup="isPopup"
    :should-render-chat="shouldRenderChat"
    :is-popup-pending="isPopupPending"
    :should-show-context-loading="shouldShowContextLoading"
    :is-context-loading="isContextLoading"
    :file-drop-active="fileDropActive"
    :timeline-events="timelineEvents"
    :empty-headline="emptyHeadline"
    :is-turn-running="isTurnRunning"
    :project-cwd="project?.cwd ?? null"
    :context-search-cwd="contextSearchCwd"
    :active-plan-approval-turn-id="pendingPlanApproval?.turnId ?? null"
    :user-send-scroll-key="userSendScrollKey"
    :restore-draft-key="restoreDraftKey"
    :restore-draft-content="restoreDraftContent"
    :pending-agent-actions="pendingAgentActions"
    :show-expired-pending-actions="nonInterruptMode"
    :can-retry-event="canRetryEvent"
    :composer-state="composerForView"
    :attachments="attachments"
    :append-attachments-to-end-key="droppedAttachmentAppendKey"
    :pending-ask="nonInterruptMode ? null : pendingAskUser"
    :tool-consent="nonInterruptMode ? null : pendingToolConsent"
    :viewing-image="viewingImage"
    :suggestions="suggestions"
    :suggestions-visible="shouldLoadSuggestions"
    @resolve-pending-agent-action="composerController.onResolvePendingAgentAction"
    @retry-event="composerController.onRetryTimelineEvent"
    @open-image="viewingImage = $event"
    @close-image="viewingImage = null"
    @insert-guide="composerController.onInsertGuide"
    @send="composerController.onSend"
    @interrupt="composerController.onInterrupt"
    @update-composer="composerController.onComposerUpdate"
    @remove-attachment="attachmentController.removeAttachment"
    @pick-attachments="attachmentController.onPickAttachments"
    @add-context-attachment="attachmentController.addContextAttachment"
    @resolve-ask-user="composerController.onResolveAskUser"
    @resolve-tool-consent="composerController.onResolveToolConsent"
  />
</template>
