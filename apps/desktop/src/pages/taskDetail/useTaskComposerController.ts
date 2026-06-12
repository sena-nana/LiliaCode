import { computed, nextTick, ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isAgentTimelineToolWindowKind } from "@lilia/contracts";
import type { ChatAttachment, ChatComposerState, ChatWorkflow, CodexThreadGoal } from "@lilia/contracts";
import {
  useAskUserForTask,
  usePendingAsksForTask,
} from "../../composables/useAskUser";
import {
  usePendingAgentActionsForTask,
} from "../../composables/usePendingAgentActions";
import {
  usePendingCodexInteractionsForTask,
} from "../../composables/useCodexPendingInteractions";
import {
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
  getRuntimeSnapshot,
  ackRestoredRollback,
  interruptTurn,
  onAgentTimeline,
  onAgentTimelineBatch,
  onDone,
  onTurnStarted,
  sendMessage,
  setComposerState,
} from "../../services/chat";
import { serializeAttachmentReference } from "../../components/chat/composerParts";
import type { TaskTodo } from "../../services/todos";
import type { TaskDetailRouteProps, useTaskConversationContext } from "./useTaskConversationContext";
import type { useTaskTimeline } from "./useTaskTimeline";
import { useCodexWorkflowActions } from "./useCodexWorkflowActions";
import {
  clearPendingInteractionsForTask,
  hydratePendingInteractions,
  usePendingInteractionActions,
} from "./usePendingInteractionActions";
import type { ChatRuntimePhase } from "@lilia/contracts";

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
  const insertDraftTextKey = ref(0);
  const insertDraftTextContent = ref("");
  const pendingAskUser = useAskUserForTask(() => props.taskId);
  const pendingAskUsers = usePendingAsksForTask(() => props.taskId);
  const pendingToolConsent = useToolConsentForTask(() => props.taskId);
  const pendingToolConsents = usePendingToolConsentsForTask(() => props.taskId);
  const pendingCodexInteractions = usePendingCodexInteractionsForTask(() => props.taskId);
  const runtimePendingAgentActions = usePendingAgentActionsForTask(
    pendingAskUsers,
    pendingToolConsents,
    pendingCodexInteractions,
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
      nonInterruptMode.value ||
      action.kind === "title_update" ||
      action.kind === "mcp_elicitation" ||
      action.kind === "permission_approval"
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
  const currentCodexGoal = computed<CodexThreadGoal | null>(() =>
    latestCodexGoalFromTimeline(timeline.timelineEvents.value),
  );

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
    workflow?: ChatWorkflow | null,
  ) {
    if (!context.hasContext.value) return;
    if (!content.trim() && outgoingAttachments.length === 0 && !workflow) return;

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
        workflow,
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

  const codexWorkflowActions = useCodexWorkflowActions({
    hasContext: context.hasContext,
    isTurnRunning,
    blockingPendingAgentActions,
    attachments,
    sendAgentMessage,
  });

  function onInsertGuide(todo: TaskTodo) {
    void guideDispatch.dispatchGuide(todo);
  }

  function onInsertDraftText(text: string) {
    if (!text) return;
    insertDraftTextContent.value = text;
    insertDraftTextKey.value += 1;
  }

  async function onInterrupt() {
    if (!context.hasContext.value || !isTurnRunning.value) return;
    try {
      const result = await interruptTurn(props.taskId);
      if (agentInteractionSettings.agentRuntimeChannel.value === "mutsuki_core") return;
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

  const pendingInteractionActions = usePendingInteractionActions({
    taskId: () => props.taskId,
    pendingAskUser,
    pendingToolConsent,
    pendingToolConsents,
    pendingCodexInteractions,
  });

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

  function runtimePhaseKeepsTurnRunning(phase: ChatRuntimePhase): boolean {
    return phase === "running" ||
      phase === "running_and_queued" ||
      phase === "interrupted_pending_finish" ||
      phase === "reset_pending_finish";
  }

  function runtimeAbandonedMessage(snapshot: Awaited<ReturnType<typeof getRuntimeSnapshot>>): string {
    const backend = snapshot.backend ?? "agent";
    const runtime = snapshot.runtimeChannel ?? "unknown";
    const turn = snapshot.turnId ? `，turn=${snapshot.turnId}` : "";
    return `上次 ${backend} 运行未正常结束，${runtime} runtime 已放弃旧执行态${turn}。你可以重新发送或重置会话。`;
  }

  function restoreDraftFromRollback(rollback: { restoredContent: string; restoredAttachments: ChatAttachment[] }) {
    restoreDraftContent.value = stripRestoredAttachmentReferences(
      rollback.restoredContent,
      rollback.restoredAttachments,
    );
    restoreDraftKey.value += 1;
    attachments.value = rollback.restoredAttachments;
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
    const runtimeSnapshotLoad = getRuntimeSnapshot(taskId).catch((err) => {
      console.error("[chat] getRuntimeSnapshot failed", err);
      return null;
    });
    if (!existingComposerLoad) composerLoad = nextComposerLoad;
    try {
      const [events, comp, runtimeSnapshot] = await Promise.all([
        timeline.loadTimelineEvents(taskId),
        nextComposerLoad,
        runtimeSnapshotLoad,
      ]);
      if (seq !== loadSeq || taskId !== props.taskId || projectId !== props.projectId) return;
      timeline.applyLoadedTimelineEvents(events);
      const activeRequestIds = hydratePendingInteractions(events, props.taskId);
      clearPendingInteractionsForTask(taskId, { keepRequestIds: activeRequestIds });
      isTurnRunning.value = runtimeSnapshot
        ? runtimePhaseKeepsTurnRunning(runtimeSnapshot.phase)
        : false;
      if (runtimeSnapshot?.phase === "abandoned") {
        clearPendingInteractionsForTask(taskId);
        timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent(
          runtimeAbandonedMessage(runtimeSnapshot),
        ));
      }
      if (runtimeSnapshot?.rollback?.rolledBack) {
        restoreDraftFromRollback(runtimeSnapshot.rollback);
        void ackRestoredRollback(taskId);
      }
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
        hydratePendingInteractions([e], props.taskId);
        if (isAgentTimelineToolWindowKind(e.kind)) {
          void guideDispatch.scheduleGuideInsertion("tool");
        }
      }),
      onAgentTimelineBatch((e) => {
        if (e.taskId !== props.taskId) return;
        timeline.queueTimelineEvents(e.events);
        hydratePendingInteractions(e.events, props.taskId);
        if (e.events.some((event) => isAgentTimelineToolWindowKind(event.kind))) {
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
        clearPendingInteractionsForTask(e.taskId);
        if (e.rollback?.rolledBack) {
          for (const eventId of e.rollback.removedEventIds) {
            timeline.removeTimelineEvent(eventId);
          }
          restoreDraftFromRollback(e.rollback);
        }
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
    insertDraftTextKey,
    insertDraftTextContent,
    pendingAskUser,
    pendingAskUsers,
    pendingToolConsent,
    pendingToolConsents,
    pendingAgentActions,
    blockingPendingAgentActions,
    pendingPlanApproval,
    currentCodexGoal,
    agentInteractionSettings,
    nonInterruptMode,
    activeBackend,
    sendAgentMessage,
    onSend,
    ...codexWorkflowActions,
    onInsertGuide,
    onInsertDraftText,
    onInterrupt,
    ...pendingInteractionActions,
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
    next = next.split(serializeAttachmentReference(attachment)).join("");
  }
  return next.replace(/[ \t]{2,}/g, " ").trim();
}

function latestCodexGoalFromTimeline(
  events: readonly { kind: string; payload: unknown; updatedAt: number }[],
): CodexThreadGoal | null {
  let latest: { payload: unknown; updatedAt: number } | null = null;
  for (const event of events) {
    if (event.kind !== "goal") continue;
    if (!latest || event.updatedAt >= latest.updatedAt) latest = event;
  }
  if (!latest) return null;
  const payload = latest.payload;
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const row = payload as Record<string, unknown>;
  if (row.cleared === true) return null;
  const goal = row.goal;
  if (!goal || typeof goal !== "object" || Array.isArray(goal)) return null;
  return goal as CodexThreadGoal;
}
