import { computed, nextTick, ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isAgentTimelineToolWindowKind } from "@lilia/contracts";
import type {
  AskUserResult,
  ChatAttachment,
  ChatComposerState,
} from "@lilia/contracts";
import {
  resolveAskUserById,
  useAskUserForTask,
  usePendingAsksForTask,
} from "../../composables/useAskUser";
import {
  usePendingAgentActionsForTask,
  type PendingAgentActionResolution,
} from "../../composables/usePendingAgentActions";
import {
  respondConsent,
  usePendingToolConsentsForTask,
  useToolConsentForTask,
} from "../../composables/useToolConsentBridge";
import { useGuideDispatch } from "../../composables/useGuideDispatch";
import {
  loadAgentInteractionSettings,
  useAgentInteractionSettings,
} from "../../composables/useAgentInteractionSettings";
import { useConnectionStatus } from "../../composables/useConnectionStatus";
import {
  getComposerState,
  interruptTurn,
  onAgentTimeline,
  onDone,
  onTurnStarted,
  respondTitleUpdate,
  sendMessage,
  setComposerState,
  type ToolConsentDecision,
  type ToolConsentUpdatedInput,
} from "../../services/chat";
import { serializeAttachmentReference } from "../../components/chat/composerParts";
import type { TaskTodo } from "../../services/todos";
import type { TaskDetailRouteProps, useTaskConversationContext } from "./useTaskConversationContext";
import type { useTaskTimeline } from "./useTaskTimeline";

export function useTaskComposerController(options: {
  props: TaskDetailRouteProps;
  context: ReturnType<typeof useTaskConversationContext>;
  timeline: ReturnType<typeof useTaskTimeline>;
  attachments: Ref<ChatAttachment[]>;
}) {
  const { props, context, timeline, attachments } = options;
  const composer = ref<ChatComposerState | null>(null);
  const isTurnRunning = ref(false);
  const userSendScrollKey = ref(0);
  const restoreDraftKey = ref(0);
  const restoreDraftContent = ref("");
  const pendingAskUser = useAskUserForTask(() => props.taskId);
  const pendingAskUsers = usePendingAsksForTask(() => props.taskId);
  const pendingToolConsent = useToolConsentForTask(() => props.taskId);
  const pendingToolConsents = usePendingToolConsentsForTask(() => props.taskId);
  const runtimePendingAgentActions = usePendingAgentActionsForTask(
    pendingAskUsers,
    pendingToolConsents,
    timeline.timelineEvents,
  );
  const agentInteractionSettings = useAgentInteractionSettings();
  const nonInterruptMode = agentInteractionSettings.nonInterruptMode;
  const { activeBackend } = useConnectionStatus({ probe: !context.isPopup.value });
  let loadSeq = 0;
  let composerLoad: Promise<ChatComposerState | null> | null = null;

  const composerForView = computed<ChatComposerState>(() =>
    withActiveBackend(composer.value ?? {
      taskId: props.taskId,
      backend: activeBackend.value,
      model: "",
      planMode: false,
      permission: "ask",
    }),
  );
  const pendingAgentActions = computed(() =>
    runtimePendingAgentActions.value.filter((action) =>
      nonInterruptMode.value || action.kind === "title_update"
    ),
  );
  const blockingPendingAgentActions = computed(() =>
    pendingAgentActions.value.filter((action) => action.kind !== "title_update"),
  );
  const pendingPlanApproval = computed(() => {
    const ask = nonInterruptMode.value
      ? pendingAskUsers.value.find((item) => item.spec.intent === "plan_approval") ?? null
      : pendingAskUser.value;
    if (!ask) return null;
    if (ask.spec.intent !== "plan_approval") return null;
    const question = ask.spec.questions[0];
    return question ? { questionId: question.id, turnId: ask.turnId } : null;
  });

  function withActiveBackend(state: ChatComposerState): ChatComposerState {
    return {
      ...state,
      taskId: props.taskId,
      backend: activeBackend.value,
    };
  }

  async function sendAgentMessage(
    content: string,
    outgoingAttachments: ChatAttachment[] = [],
    guideId?: string,
  ) {
    if (!context.hasContext.value) return;
    if (!content.trim() && outgoingAttachments.length === 0) return;

    let optimisticId: string | null = null;
    try {
      await ensureComposerLoaded();
      const currentComposer = composerForView.value;
      await context.ensureTaskReadyForMessage(content, outgoingAttachments);
      const cwd = context.project.value?.cwd ?? (await context.ensureOrphanCwd());

      const optimistic = timeline.createOptimisticMessageEvent({
        content,
        attachments: outgoingAttachments,
      });
      optimisticId = optimistic.id;
      timeline.upsertTimelineEvent(optimistic);
      userSendScrollKey.value += 1;
      await sendMessage(
        props.taskId,
        content,
        currentComposer,
        cwd,
        outgoingAttachments,
        guideId,
      );
      timeline.removeTimelineEvent(optimistic.id);
    } catch (err) {
      if (optimisticId) timeline.removeTimelineEvent(optimisticId);
      isTurnRunning.value = false;
      timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent(`发送失败：${String(err)}`, {
        content,
        attachments: outgoingAttachments,
      }));
      throw err;
    }
  }

  async function onSend(content: string, outgoingAttachments: ChatAttachment[] = []) {
    if (!context.hasContext.value) return;
    if (isTurnRunning.value || blockingPendingAgentActions.value.length > 0) {
      await guideDispatch.createGuideFromComposer(content, outgoingAttachments);
      return;
    }

    try {
      await sendAgentMessage(content, outgoingAttachments);
      attachments.value = [];
    } catch {
      // sendAgentMessage 已经把失败写入 timeline；这里吞掉异常避免 Vue 事件处理链重复报错。
    }
  }

  function onInsertGuide(todo: TaskTodo) {
    void guideDispatch.dispatchGuide(todo);
  }

  async function onInterrupt() {
    if (!context.hasContext.value || !isTurnRunning.value) return;
    try {
      const result = await interruptTurn(props.taskId);
      if (!result.rolledBack) return;
      isTurnRunning.value = false;
      for (const eventId of result.removedEventIds) {
        timeline.removeTimelineEvent(eventId);
      }
      restoreDraftContent.value = stripRestoredAttachmentReferences(
        result.restoredContent,
        result.restoredAttachments,
      );
      restoreDraftKey.value += 1;
      await nextTick();
      attachments.value = result.restoredAttachments;
    } catch (err) {
      timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent(`打断失败：${String(err)}`));
    }
  }

  function onResolveAskUser(result: AskUserResult) {
    const ask = pendingAskUser.value;
    if (!ask) return;
    resolveAskUserById(ask.id, result);
  }

  async function onResolveToolConsent(
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ) {
    const request = pendingToolConsent.value;
    if (!request) return;
    try {
      await respondConsent(request.taskId, request.requestId, decision, message, updatedInput);
    } catch (err) {
      console.error("[tool-consent] respond failed", err);
    }
  }

  async function onResolvePendingAgentAction(resolution: PendingAgentActionResolution) {
    if (resolution.kind === "title_update") {
      try {
        await respondTitleUpdate(props.taskId, resolution.requestId, resolution.decision);
      } catch (err) {
        console.error("[title-update] respond failed", err);
      }
      return;
    }
    if (resolution.kind === "tool_consent") {
      const request = pendingToolConsents.value.find(
        (item) => item.requestId === resolution.requestId,
      );
      if (!request) return;
      try {
        await respondConsent(
          request.taskId,
          request.requestId,
          resolution.decision,
          resolution.message,
          resolution.updatedInput,
        );
      } catch (err) {
        console.error("[tool-consent] respond failed", err);
      }
      return;
    }
    resolveAskUserById(resolution.askId, resolution.result);
  }

  async function onComposerUpdate(next: ChatComposerState) {
    const normalized = withActiveBackend(next);
    composer.value = normalized;
    try {
      await setComposerState(normalized);
    } catch (err) {
      console.error("[chat] setComposerState failed", err);
    }
  }

  async function onRetryTimelineEvent(event: Parameters<typeof timeline.retryContextForEvent>[0]) {
    const retryContext = timeline.retryContextForEvent(event);
    if (!retryContext) return;
    try {
      await sendAgentMessage(retryContext.content, retryContext.attachments);
    } catch (err) {
      console.error("[chat] retry failed", err);
    }
  }

  async function loadComposerForCurrentTask(
    taskId: string,
    seq: number,
  ): Promise<ChatComposerState | null> {
    const comp = await getComposerState(taskId);
    if (seq !== loadSeq || taskId !== props.taskId) return null;
    composer.value = withActiveBackend(comp);
    return composer.value;
  }

  async function loadComposerForSend(taskId: string): Promise<ChatComposerState | null> {
    const comp = await getComposerState(taskId);
    if (taskId !== props.taskId) return null;
    composer.value = withActiveBackend(comp);
    return composer.value;
  }

  async function ensureComposerLoaded(): Promise<ChatComposerState> {
    if (composer.value) return composer.value;
    const taskId = props.taskId;
    if (!composerLoad) {
      const directLoad = loadComposerForSend(taskId).catch((err) => {
        console.error("[chat] getComposerState failed", err);
        return null;
      });
      composerLoad = directLoad;
    }
    const pending = composerLoad;
    let loaded = await pending;
    if (composerLoad === pending) composerLoad = null;
    if (!loaded && taskId === props.taskId) {
      loaded = await loadComposerForSend(taskId);
    }
    if (!loaded) throw new Error("Composer 尚未就绪");
    return loaded;
  }

  async function loadAll() {
    const seq = ++loadSeq;
    const taskId = props.taskId;
    const projectId = props.projectId;
    if (context.isPopup.value && !context.conversationRouteState.value.isLiveDraft) {
      context.popupContentReady.value = false;
    }
    composer.value = null;
    const existingComposerLoad = composerLoad;
    const nextComposerLoad = existingComposerLoad ??
      loadComposerForCurrentTask(taskId, seq).catch((err) => {
        console.error("[chat] getComposerState failed", err);
        return null;
      });
    if (!existingComposerLoad) composerLoad = nextComposerLoad;
    try {
      const [events, comp] = await Promise.all([
        timeline.loadTimelineEvents(taskId),
        nextComposerLoad,
      ]);
      if (seq !== loadSeq || taskId !== props.taskId || projectId !== props.projectId) return;
      timeline.applyLoadedTimelineEvents(events);
      if (!comp) return;
    } finally {
      if (composerLoad === nextComposerLoad) composerLoad = null;
      if (seq === loadSeq && taskId === props.taskId && projectId === props.projectId) {
        if (context.isPopup.value) context.popupContentReady.value = true;
      }
    }
  }

  async function installRuntimeListeners(): Promise<UnlistenFn[]> {
    return await Promise.all([
      onAgentTimeline((e) => {
        if (e.taskId !== props.taskId) return;
        timeline.upsertTimelineEvent(e);
        if (isAgentTimelineToolWindowKind(e.kind)) {
          void guideDispatch.scheduleGuideInsertion("tool");
        }
      }),
      onTurnStarted((e) => {
        if (e.taskId !== props.taskId) return;
        isTurnRunning.value = true;
        timeline.markQueuedUserMessageSuccessful();
      }),
      onDone((e) => {
        if (e.taskId !== props.taskId) return;
        isTurnRunning.value = false;
        void guideDispatch.scheduleGuideInsertion("idle");
      }),
    ]);
  }

  function resetForRouteChange() {
    isTurnRunning.value = false;
    composer.value = null;
  }

  function canAcceptInteractiveDrop(): boolean {
    return nonInterruptMode.value || (!pendingAskUser.value && !pendingToolConsent.value);
  }

  const guideDispatch = useGuideDispatch({
    taskId: () => props.taskId,
    ensureReady: context.ensureTaskReadyForMessage,
    sendAgentMessage,
    ensureDispatchReady: async () => {
      await ensureComposerLoaded();
    },
    hasPendingAgentAction: () =>
      pendingAskUsers.value.length > 0 || pendingToolConsents.value.length > 0,
    isTurnRunning: () => isTurnRunning.value,
    clearAttachments: () => {
      attachments.value = [];
    },
    reportError: (message) => {
      timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent(message));
    },
  });

  return {
    composer,
    composerForView,
    isTurnRunning,
    userSendScrollKey,
    restoreDraftKey,
    restoreDraftContent,
    pendingAskUser,
    pendingAskUsers,
    pendingToolConsent,
    pendingToolConsents,
    pendingAgentActions,
    blockingPendingAgentActions,
    pendingPlanApproval,
    agentInteractionSettings,
    nonInterruptMode,
    activeBackend,
    sendAgentMessage,
    onSend,
    onInsertGuide,
    onInterrupt,
    onResolveAskUser,
    onResolveToolConsent,
    onResolvePendingAgentAction,
    onComposerUpdate,
    onRetryTimelineEvent,
    loadAll,
    loadAgentInteractionSettings,
    installRuntimeListeners,
    resetForRouteChange,
    canAcceptInteractiveDrop,
    scheduleUserGuideInsertion: () => guideDispatch.scheduleGuideInsertion("user"),
  };
}

function stripRestoredAttachmentReferences(
  content: string,
  attachments: ChatAttachment[],
): string {
  let next = content;
  for (const attachment of attachments) {
    next = next.replaceAll(serializeAttachmentReference(attachment), "");
  }
  return next.replace(/[ \t]{2,}/g, " ").trim();
}
