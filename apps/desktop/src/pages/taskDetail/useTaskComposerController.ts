import { computed, nextTick, ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isAgentTimelineToolWindowKind } from "@lilia/contracts";
import type {
  ChatAttachment,
  ChatContextUsage,
  ChatComposerState,
  ChatSlashCommandWorkflow,
  LiliaThreadGoal,
} from "@lilia/contracts";
import {
  useAskUserForTask,
  usePendingAsksForTask,
} from "../../composables/useAskUser";
import {
  usePendingAgentActionsForTask,
} from "../../composables/usePendingAgentActions";
import {
  usePendingAgentInteractionsForTask,
} from "../../composables/useAgentPendingInteractions";
import {
  usePendingProjectArchitectureChangesForTask,
} from "../../composables/useProjectArchitectureInteractions";
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
  onContextUsage,
  onDone,
  onTurnStarted,
  sendMessage,
  setComposerState,
} from "../../services/chat";
import type { SendMessageInput } from "../../services/chat";
import { serializeAttachmentReference } from "../../components/chat/composerParts";
import type { TaskTodo } from "../../services/todos";
import type { TaskDetailRouteProps, useTaskConversationContext } from "./useTaskConversationContext";
import type { useTaskTimeline } from "./useTaskTimeline";
import { useLiliaWorkflowActions } from "./useLiliaWorkflowActions";
import type { LiliaWorkflowSendAgentMessageInput } from "./useLiliaWorkflowActions";
import {
  clearPendingInteractionsForTask,
  hydratePendingInteractions,
  usePendingInteractionActions,
} from "./usePendingInteractionActions";
import type { ChatRuntimePhase } from "@lilia/contracts";

type SendAgentMessageInput = LiliaWorkflowSendAgentMessageInput;

export function useTaskComposerController(options: {
  props: TaskDetailRouteProps;
  context: ReturnType<typeof useTaskConversationContext>;
  timeline: ReturnType<typeof useTaskTimeline>;
  attachments: Ref<ChatAttachment[]>;
}) {
  const { props, context, timeline, attachments } = options;
  const composer = ref<ChatComposerState | null>(null);
  const contextUsage = ref<ChatContextUsage | null>(null);
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
  const pendingAgentInteractions = usePendingAgentInteractionsForTask(() => props.taskId);
  const pendingArchitectureChanges = usePendingProjectArchitectureChangesForTask(() => props.taskId);
  const runtimePendingAgentActions = usePendingAgentActionsForTask(
    pendingAskUsers,
    pendingToolConsents,
    pendingAgentInteractions,
    timeline.timelineEvents,
    pendingArchitectureChanges,
  );
  const agentInteractionSettings = useAgentInteractionSettings();
  const nonInterruptMode = agentInteractionSettings.nonInterruptMode;
  const { activeBackend } = useConnectionStatus({
    probe: !context.isPopup.value && !context.conversationRouteState.value.isLiveDraft,
  });
  let loadSeq = 0;
  let runtimeEventSeq = 0;
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
      action.kind === "permission_approval" ||
      action.kind === "architecture_change"
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
  const currentLiliaGoal = computed<LiliaThreadGoal | null>(() =>
    latestLiliaGoalFromTimeline(timeline.timelineEvents.value),
  );

  function withActiveBackend(state: ChatComposerState): ChatComposerState {
    return {
      ...state,
      taskId: props.taskId,
      backend: activeBackend.value,
    };
  }

  async function sendAgentMessage(input: SendAgentMessageInput) {
    if (!context.hasContext.value) return;
    const outgoingAttachments = input.turn.outgoingAttachments ?? [];
    const workflow = input.workflow ?? null;
    const runtimeCommand = input.runtimeCommand ?? null;
    const runtimeOptions = input.runtimeOptions ?? null;
    const titleContent = input.turn.titleContent;
    const content = input.turn.content;
    if (!content.trim() && outgoingAttachments.length === 0 && !workflow && !runtimeCommand) return;

    let optimisticId: string | null = null;
    try {
      await ensureComposerLoaded();
      const currentComposer = composerForView.value;
      await context.ensureTaskReadyForMessage(titleContent ?? content, outgoingAttachments);
      const cwd = context.project.value?.cwd ?? (await context.ensureOrphanCwd());

      const optimistic = timeline.createOptimisticMessageEvent({
        content,
        attachments: outgoingAttachments,
      });
      optimisticId = optimistic.id;
      timeline.upsertTimelineEvent(optimistic);
      userSendScrollKey.value += 1;
      const sendInput: SendMessageInput = {
        taskId: props.taskId,
        turn: {
          content,
          composer: currentComposer,
          projectCwd: cwd,
          attachments: outgoingAttachments,
          guideId: input.turn.guideId ?? null,
        },
        workflow,
        runtimeCommand,
        runtimeOptions,
      };
      await sendMessage(sendInput);
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
      await sendAgentMessage({ turn: { content, outgoingAttachments } });
      attachments.value = [];
    } catch {
      // sendAgentMessage 已经把失败写入 timeline；这里吞掉异常避免 Vue 事件处理链重复报错。
    }
  }

  async function onExecuteSlashCommand(workflow: ChatSlashCommandWorkflow) {
    if (!context.hasContext.value) return;
    if (isTurnRunning.value || blockingPendingAgentActions.value.length > 0) {
      timeline.upsertTimelineEvent(timeline.createLocalErrorTimelineEvent("当前 Agent 正在运行，暂不能执行斜杠命令。"));
      return;
    }
    try {
      const commandName = workflow.commandId.split(":").at(1) ?? workflow.commandId;
      await sendAgentMessage({
        turn: {
          content: "",
          outgoingAttachments: [],
          titleContent: `/${commandName}`,
        },
        workflow,
      });
    } catch {
      // sendAgentMessage 已写入本地错误 timeline。
    }
  }

  const liliaWorkflowActions = useLiliaWorkflowActions({
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
    pendingAgentInteractions,
    pendingArchitectureChanges,
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
      await sendAgentMessage({
        turn: {
          content: retryContext.content,
          outgoingAttachments: retryContext.attachments,
        },
      });
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
    const turn = snapshot.turnId ? `，turn=${snapshot.turnId}` : "";
    return `上次 ${backend} 运行未正常结束，旧执行态已放弃${turn}。你可以重新发送或重置会话。`;
  }

  function restoreDraftFromRollback(rollback: { restoredContent: string; restoredAttachments: ChatAttachment[] }) {
    restoreDraftContent.value = stripRestoredAttachmentReferences(
      rollback.restoredContent,
      rollback.restoredAttachments,
    );
    restoreDraftKey.value += 1;
    attachments.value = rollback.restoredAttachments;
  }

  function currentPendingRequestIds(): Set<string> {
    const ids = new Set<string>();
    for (const ask of pendingAskUsers.value) {
      if (ask.requestId) ids.add(ask.requestId);
    }
    for (const consent of pendingToolConsents.value) {
      ids.add(consent.requestId);
    }
    for (const interaction of pendingAgentInteractions.value) {
      ids.add(interaction.requestId);
    }
    for (const change of pendingArchitectureChanges.value) {
      ids.add(change.requestId);
    }
    return ids;
  }

  async function loadAll() {
    const seq = ++loadSeq;
    const taskId = props.taskId;
    const projectId = props.projectId;
    const pendingBeforeLoad = currentPendingRequestIds();
    const timelineEventIdsBeforeLoad = new Set(
      timeline.persistedTimelineEvents.value.map((event) => event.id),
    );
    const runtimeSeqBeforeLoad = runtimeEventSeq;
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
      const timelineEventIdsToPreserve = new Set(
        timeline.persistedTimelineEvents.value
          .filter((event) => !timelineEventIdsBeforeLoad.has(event.id))
          .map((event) => event.id),
      );
      timeline.applyLoadedTimelineEvents(events, timelineEventIdsToPreserve);
      const activeRequestIds = hydratePendingInteractions(events, props.taskId);
      for (const requestId of currentPendingRequestIds()) {
        if (!pendingBeforeLoad.has(requestId)) activeRequestIds.add(requestId);
      }
      clearPendingInteractionsForTask(taskId, { keepRequestIds: activeRequestIds });
      if (runtimeSeqBeforeLoad === runtimeEventSeq) {
        contextUsage.value = runtimeSnapshot?.contextUsage ?? null;
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
        runtimeEventSeq += 1;
        isTurnRunning.value = true;
        timeline.markQueuedUserMessageSuccessful();
      }),
      onDone((e) => {
        if (e.taskId !== props.taskId) return;
        runtimeEventSeq += 1;
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
      onContextUsage((e) => {
        if (e.taskId !== props.taskId) return;
        contextUsage.value = e;
      }),
    ]);
  }

  function resetForRouteChange() {
    isTurnRunning.value = false;
    composer.value = null;
    contextUsage.value = null;
  }

  function canAcceptInteractiveDrop(): boolean {
    return nonInterruptMode.value || (!pendingAskUser.value && !pendingToolConsent.value);
  }

  const guideDispatch = useGuideDispatch({
    taskId: () => props.taskId,
    ensureReady: context.ensureTaskReadyForMessage,
    sendAgentMessage: (content, outgoingAttachments, guideId) =>
      sendAgentMessage({ turn: { content, outgoingAttachments, guideId } }),
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
    contextUsage,
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
    currentLiliaGoal,
    agentInteractionSettings,
    nonInterruptMode,
    activeBackend,
    sendAgentMessage,
    onSend,
    onExecuteSlashCommand,
    ...liliaWorkflowActions,
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

function latestLiliaGoalFromTimeline(
  events: readonly { kind: string; payload: unknown; updatedAt: number }[],
): LiliaThreadGoal | null {
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
  return goal as LiliaThreadGoal;
}
