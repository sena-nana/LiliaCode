<script setup lang="ts">
import "../styles/chat.css";
/**
 * Task 详情 = 聊天面板。此页只编排路由上下文、timeline、composer 与附件控制器；
 * 具体聊天布局由 TaskDetailChatSurface 负责。
 */

import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useRoute, useRouter } from "vue-router";
import type { ChatAttachment, SuggestionItem } from "@lilia/contracts";
import { registerArchitectureChatSidebarPanel } from "../composables/useArchitectureChatSidebarPanel";
import { registerDebugChatSidebarPanel } from "../composables/useDebugChatSidebarPanel";
import TaskDetailChatSurface from "./taskDetail/TaskDetailChatSurface.vue";
import { useSidebarDisplayMode } from "../composables/useSidebarDisplayMode";
import { useTaskAttachments } from "./taskDetail/useTaskAttachments";
import { useTaskComposerController } from "./taskDetail/useTaskComposerController";
import {
  useTaskConversationContext,
  type TaskDetailRouteProps,
} from "./taskDetail/useTaskConversationContext";
import { useTaskTimeline } from "./taskDetail/useTaskTimeline";
import { createDraftTask } from "../services/tasksStore";
import { listProjects } from "../services/projectsStore";
import { getConversationSuggestions } from "../services/chat";

const props = withDefaults(defineProps<{
  projectId?: string;
  taskId: string;
  variant?: "main" | "popup";
}>(), {
  variant: "main",
});

const routeProps = props as TaskDetailRouteProps;
const route = useRoute();
const router = useRouter();
const chatSurfaceRef = ref<InstanceType<typeof TaskDetailChatSurface> | null>(null);
const chatPageRef = computed<HTMLElement | null>(() =>
  chatSurfaceRef.value?.chatPageRef ?? null,
);
const focusConversationKey = computed(() =>
  shouldRenderChat.value
    ? `${props.variant ?? "main"}:${props.projectId ?? ""}:${props.taskId}`
    : "",
);
const sharedAttachments = ref<ChatAttachment[]>([]);
const suggestions = ref<SuggestionItem[]>([]);
const { sidebarDisplayMode } = useSidebarDisplayMode();
type SuggestionsStatus = "idle" | "loading" | "empty" | "error";
const suggestionsStatus = ref<SuggestionsStatus>("idle");
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
  insertDraftTextKey,
  insertDraftTextContent,
  pendingAskUser,
  pendingToolConsent,
  pendingAgentActions,
  blockingPendingAgentActions,
  pendingPlanApproval,
  nonInterruptMode,
} = composerController;

function queryString(value: unknown): string | null {
  if (Array.isArray(value)) return typeof value[0] === "string" ? value[0] : null;
  return typeof value === "string" ? value : null;
}

function decodeDraftTextParam(value: string | null): string {
  if (!value) return "";
  try {
    const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
    const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch (err) {
    console.error("[popup] decode initial draft failed", err);
    return "";
  }
}

const isIdleEmptyDraft = computed(() =>
  shouldRenderChat.value &&
  conversationRouteState.value.isLiveDraft &&
  timelineEvents.value.length === 0 &&
  !isTurnRunning.value &&
  blockingPendingAgentActions.value.length === 0 &&
  !pendingAskUser.value &&
  !pendingToolConsent.value,
);
const shouldLoadSuggestions = computed(() =>
  !!props.projectId &&
  isIdleEmptyDraft.value,
);
const draftProjectPickerProjects = computed(() => listProjects());
const shouldShowDraftProjectPicker = computed(() =>
  sidebarDisplayMode.value === "unified" &&
  !props.projectId &&
  isIdleEmptyDraft.value,
);

async function loadSuggestions(forceRefresh = false) {
  if (!shouldLoadSuggestions.value) {
    suggestions.value = [];
    suggestionsStatus.value = "idle";
    return;
  }
  const seq = ++suggestionsSeq;
  suggestionsStatus.value = "loading";
  try {
    const next = await getConversationSuggestions(props.projectId ?? null, forceRefresh);
    if (seq === suggestionsSeq) {
      suggestions.value = next;
      suggestionsStatus.value = next.length > 0 ? "idle" : "empty";
    }
  } catch (err) {
    console.error("[conversation-suggestions] load failed", err);
    if (seq === suggestionsSeq) {
      suggestions.value = [];
      suggestionsStatus.value = "error";
    }
  }
}

async function moveCurrentDraftToProject(projectId: string) {
  const draft = createDraftTask(projectId);
  const snapshot = chatSurfaceRef.value?.getComposerDraftSnapshot() ?? { content: "" };
  const nextAttachments = [...sharedAttachments.value];
  await router.push(`/projects/${projectId}/tasks/${draft.id}`);
  await nextTick();
  sharedAttachments.value = nextAttachments;
  if (snapshot.content.trim()) {
    composerController.onInsertDraftText(snapshot.content);
  }
}

function onDraftProjectPickerError(message: string) {
  timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent(message));
}

const unlisteners: UnlistenFn[] = [];
let unregisterDebugPanel: (() => void) | null = null;
let unregisterArchitecturePanel: (() => void) | null = null;

function syncDebugPanelRegistration() {
  if (!hasContext.value || !composerController.agentInteractionSettings.debug.value) {
    unregisterDebugPanel?.();
    unregisterDebugPanel = null;
    return;
  }
  if (unregisterDebugPanel) return;
  unregisterDebugPanel = registerDebugChatSidebarPanel();
}

function syncArchitecturePanelRegistration() {
  if (!hasContext.value || !props.projectId || isPopup.value) {
    unregisterArchitecturePanel?.();
    unregisterArchitecturePanel = null;
    return;
  }
  if (unregisterArchitecturePanel) return;
  unregisterArchitecturePanel = registerArchitectureChatSidebarPanel();
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
  unregisterArchitecturePanel?.();
  unregisterArchitecturePanel = null;
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
    suggestionsStatus.value = "idle";
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
      suggestionsStatus.value = "idle";
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
  () => [hasContext.value, props.projectId ?? "", isPopup.value] as const,
  syncArchitecturePanelRegistration,
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

watch(
  () => [props.taskId, route.query.draft] as const,
  ([taskId, draft]) => {
    if (props.variant !== "popup" || !taskId) return;
    const text = decodeDraftTextParam(queryString(draft));
    if (text) composerController.onInsertDraftText(text);
  },
  { immediate: true },
);

watch(
  focusConversationKey,
  async (key) => {
    if (!key) return;
    await nextTick();
    chatSurfaceRef.value?.focusComposer();
  },
  { immediate: true },
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
    :insert-draft-text-key="insertDraftTextKey"
    :insert-draft-text-content="insertDraftTextContent"
    :pending-agent-actions="pendingAgentActions"
    :has-blocking-pending-action="blockingPendingAgentActions.length > 0"
    :current-codex-goal="composerController.currentCodexGoal.value"
    :show-expired-pending-actions="nonInterruptMode"
    :can-retry-event="canRetryEvent"
    :composer-state="composerForView"
    :attachments="attachments"
    :append-attachments-to-end-key="droppedAttachmentAppendKey"
    :pending-ask="nonInterruptMode ? null : pendingAskUser"
    :tool-consent="nonInterruptMode ? null : pendingToolConsent"
    :viewing-image="viewingImage"
    :suggestions="suggestions"
    :suggestions-status="suggestionsStatus"
    :suggestions-visible="shouldLoadSuggestions"
    :show-draft-project-picker="shouldShowDraftProjectPicker"
    :draft-project-picker-projects="draftProjectPickerProjects"
    @resolve-pending-agent-action="composerController.onResolvePendingAgentAction"
    @retry-event="composerController.onRetryTimelineEvent"
    @open-image="viewingImage = $event"
    @close-image="viewingImage = null"
    @insert-guide="composerController.onInsertGuide"
    @set-codex-goal="composerController.onSetCodexGoal"
    @refresh-codex-goal="composerController.onRefreshCodexGoal"
    @clear-codex-goal="composerController.onClearCodexGoal"
    @insert-draft-text="composerController.onInsertDraftText"
    @send="composerController.onSend"
    @start-codex-review="composerController.onStartCodexReview"
    @start-codex-fix-suggestion="composerController.onStartCodexFixSuggestion"
    @start-codex-compact="composerController.onStartCodexCompact"
    @open-codex-iab="composerController.onOpenCodexIab"
    @submit-codex-iab="composerController.onSubmitCodexIab"
    @execute-slash-command="composerController.onExecuteSlashCommand"
    @start-codex-batch-apply="composerController.onStartCodexBatchApply"
    @interrupt="composerController.onInterrupt"
    @update-composer="composerController.onComposerUpdate"
    @remove-attachment="attachmentController.removeAttachment"
    @pick-attachments="attachmentController.onPickAttachments"
    @add-context-attachment="attachmentController.addContextAttachment"
    @resolve-ask-user="composerController.onResolveAskUser"
    @resolve-tool-consent="composerController.onResolveToolConsent"
    @refresh-suggestions="loadSuggestions(true)"
    @select-draft-project="moveCurrentDraftToProject"
    @created-draft-project="(project) => moveCurrentDraftToProject(project.id)"
    @draft-project-picker-error="onDraftProjectPickerError"
  />
</template>
