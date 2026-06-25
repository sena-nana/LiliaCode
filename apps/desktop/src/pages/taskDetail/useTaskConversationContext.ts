import { computed, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { homeDir } from "@tauri-apps/api/path";
import {
  createDraftOrphan,
  createDraftTask,
  ensureTaskLoaded,
  getOrphanConversation,
  getTask,
  promoteDraftOrphan,
  promoteDraftTask,
  resolveConversationRouteState,
} from "../../services/tasksStore";
import { ensureProjectLoaded, getProject } from "../../services/projectsStore";
import { rememberPopupLastProject } from "../../services/popupWindows";
import {
  getConversationContextSnapshotVersion,
  invalidateConversationContextSnapshot,
  isConversationContextSnapshotCurrent,
  onConversationContextSnapshotInvalidated,
  type ConversationContextInvalidationReason,
} from "../../services/conversationContextInvalidation";
import type { ChatAttachment } from "@lilia/contracts";
import { measurePerfAsync } from "../../utils/perf";

export interface TaskDetailRouteProps {
  projectId?: string;
  taskId: string;
  variant?: "main" | "popup";
}

const POPUP_CONTEXT_LOADING_NOTICE_MS = 600;

export function useTaskConversationContext(props: TaskDetailRouteProps) {
  const router = useRouter();
  const isPopup = computed(() => props.variant === "popup");
  const project = computed(() =>
    props.projectId ? getProject(props.projectId) : undefined,
  );
  const projectTask = computed(() =>
    props.projectId ? getTask(props.projectId, props.taskId) : undefined,
  );
  const orphan = computed(() =>
    props.projectId ? undefined : getOrphanConversation(props.taskId),
  );
  const popupContextHydrating = ref(props.variant === "popup");
  const popupContextHydrated = ref(false);
  const popupContentReady = ref(props.variant !== "popup");
  const contextLoadingVisible = ref(false);
  const orphanCwd = ref<string | null>(null);
  let popupContextSeq = 0;
  let contextLoadingTimer: ReturnType<typeof setTimeout> | null = null;
  let contextSnapshotVersion = getConversationContextSnapshotVersion();
  let disposed = false;

  const conversationRouteState = computed(() =>
    resolveConversationRouteState(props.projectId, props.taskId),
  );
  const hasContext = computed(() => {
    if (conversationRouteState.value.isLiveDraft) return true;
    if (!isPopup.value) return !!project.value || !!orphan.value;
    return props.projectId
      ? !!project.value && !!projectTask.value
      : !!orphan.value;
  });
  const isContextLoading = computed(() =>
    isPopup.value
      ? popupContextHydrating.value || (!popupContextHydrated.value && !hasContext.value)
      : !hasContext.value && (!!props.projectId || !!props.taskId),
  );
  const isPopupContentLoading = computed(() =>
    isPopup.value &&
    hasContext.value &&
    !conversationRouteState.value.isLiveDraft &&
    !popupContentReady.value,
  );
  const isPopupPending = computed(() =>
    isPopup.value && (isContextLoading.value || isPopupContentLoading.value),
  );
  const isBlockingLoading = computed(() =>
    isPopup.value ? isPopupPending.value : isContextLoading.value,
  );
  const shouldRenderChat = computed(() =>
    hasContext.value && (!isPopup.value || !isPopupContentLoading.value),
  );
  const shouldShowContextLoading = computed(() =>
    isPopup.value ? isPopupPending.value && contextLoadingVisible.value : isContextLoading.value,
  );
  const emptyHeadline = computed(() =>
    project.value
      ? `要在 ${project.value.name} 中构建什么？`
      : "今天想做什么？",
  );
  const contextSearchCwd = computed(() => project.value?.cwd ?? orphanCwd.value ?? null);

  function markContextSnapshotInvalidated(version: number) {
    if (version === contextSnapshotVersion) return;
    contextSnapshotVersion = version;
    popupContextSeq += 1;
    clearContextLoadingTimer();
  }

  function invalidateContextSnapshot(reason: ConversationContextInvalidationReason) {
    markContextSnapshotInvalidated(invalidateConversationContextSnapshot(reason));
  }

  function captureContextSnapshot(): number {
    return contextSnapshotVersion;
  }

  function isCurrentContextSnapshot(
    snapshotVersion: number,
    projectId = props.projectId,
    taskId = props.taskId,
  ): boolean {
    return !disposed &&
      snapshotVersion === contextSnapshotVersion &&
      isConversationContextSnapshotCurrent(snapshotVersion) &&
      projectId === props.projectId &&
      taskId === props.taskId;
  }

  function assertContextSnapshotCurrent(
    snapshotVersion: number,
    projectId = props.projectId,
    taskId = props.taskId,
  ) {
    if (!isCurrentContextSnapshot(snapshotVersion, projectId, taskId)) {
      throw new Error("会话上下文已失效，请重新操作");
    }
  }

  async function ensureOrphanCwd(): Promise<string> {
    if (orphanCwd.value) return orphanCwd.value;
    const taskId = props.taskId;
    const snapshotVersion = captureContextSnapshot();
    try {
      const cwd = await homeDir();
      if (isCurrentContextSnapshot(snapshotVersion, undefined, taskId)) {
        orphanCwd.value = cwd;
      }
    } catch {
      if (isCurrentContextSnapshot(snapshotVersion, undefined, taskId)) {
        orphanCwd.value = "";
      }
    }
    return orphanCwd.value ?? "";
  }

  function summarizeTitle(text: string): string {
    const normalized = text.replace(/\s+/g, " ").trim();
    if (normalized.length <= 30) return normalized;
    return normalized.slice(0, 30) + "…";
  }

  function titleForMessage(content: string, outgoingAttachments: ChatAttachment[]): string {
    const normalized = content.replace(/\s+/g, " ").trim();
    if (normalized) return summarizeTitle(normalized);
    return outgoingAttachments[0]?.name ?? "附件";
  }

  async function ensureTaskReadyForMessage(
    content: string,
    outgoingAttachments: ChatAttachment[],
  ) {
    const snapshotVersion = captureContextSnapshot();
    const projectId = props.projectId;
    const taskId = props.taskId;
    const routeState = resolveConversationRouteState(projectId, taskId);
    if (!routeState.isDraftRoute) return;
    if (routeState.isLostDraft) throw new Error("草稿已失效，请重新创建对话");
    if (!routeState.isLiveDraft) return;

    const title = titleForMessage(content, outgoingAttachments);
    assertContextSnapshotCurrent(snapshotVersion, projectId, taskId);
    if (projectId) {
      await ensureProjectLoaded(projectId);
      assertContextSnapshotCurrent(snapshotVersion, projectId, taskId);
      await promoteDraftTask(taskId, title);
    } else await promoteDraftOrphan(taskId, title);
    assertContextSnapshotCurrent(snapshotVersion, projectId, taskId);
  }

  async function recreatePopupDraft(projectId: string | undefined) {
    const snapshotVersion = captureContextSnapshot();
    if (!isCurrentContextSnapshot(snapshotVersion, projectId, props.taskId)) return;
    const query = router.currentRoute.value.query;
    if (projectId) {
      const draft = createDraftTask(projectId);
      if (!isCurrentContextSnapshot(snapshotVersion, projectId, props.taskId)) return;
      await router.replace({
        path: `/popup/projects/${projectId}/tasks/${draft.id}`,
        query,
      });
      return;
    }
    const draft = createDraftOrphan();
    if (!isCurrentContextSnapshot(snapshotVersion, undefined, props.taskId)) return;
    await router.replace({
      path: `/popup/chats/${draft.id}`,
      query,
    });
  }

  function clearContextLoadingTimer() {
    if (contextLoadingTimer === null) return;
    clearTimeout(contextLoadingTimer);
    contextLoadingTimer = null;
  }

  async function hydratePopupContext() {
    if (disposed) return;
    if (!isPopup.value) {
      popupContextHydrating.value = false;
      popupContextHydrated.value = true;
      return;
    }

    const seq = ++popupContextSeq;
    const projectId = props.projectId;
    const taskId = props.taskId;
    const snapshotVersion = captureContextSnapshot();
    popupContextHydrating.value = true;
    popupContextHydrated.value = false;
    try {
      if (projectId) {
        await Promise.all([
          ensureProjectLoaded(projectId),
          ensureTaskLoaded(taskId, projectId),
        ]);
      } else {
        await ensureTaskLoaded(taskId, null);
      }
    } catch (err) {
      console.error("[popup] hydrate context failed", err);
    } finally {
      if (
        isCurrentContextSnapshot(snapshotVersion, projectId, taskId) &&
        seq === popupContextSeq
      ) {
        popupContextHydrating.value = false;
        popupContextHydrated.value = true;
        const routeState = resolveConversationRouteState(projectId, taskId);
        if (routeState.isLostDraft) {
          void recreatePopupDraft(projectId);
        }
      }
    }
  }

  async function hydrateMainContext() {
    if (isPopup.value) return;
    const projectId = props.projectId;
    const taskId = props.taskId;
    try {
      if (projectId) {
        await measurePerfAsync(
          "task-detail.main-context.project",
          async () => {
            await ensureProjectLoaded(projectId);
          },
          { detail: `${projectId}:${taskId}` },
        );
      } else {
        await measurePerfAsync(
          "task-detail.main-context.orphan",
          async () => {
            await ensureTaskLoaded(taskId, null);
          },
          { detail: taskId },
        );
      }
    } catch (err) {
      console.error("[chat] hydrate context failed", err);
    }
  }

  async function hydrateMainTaskRecord() {
    if (isPopup.value || !props.projectId) return;
    const projectId = props.projectId;
    const taskId = props.taskId;
    try {
      await measurePerfAsync(
        "task-detail.main-context.task-record",
        async () => {
          await ensureTaskLoaded(taskId, projectId);
        },
        { detail: `${projectId}:${taskId}` },
      );
    } catch (err) {
      console.error("[chat] hydrate task record failed", err);
    }
  }

  function prepareForRouteChange() {
    invalidateContextSnapshot("route-change");
    popupContentReady.value = !isPopup.value || conversationRouteState.value.isLiveDraft;
  }

  const stopContextInvalidationListener = onConversationContextSnapshotInvalidated((version) => {
    markContextSnapshotInvalidated(version);
  });

  watch(
    () => [props.variant, props.projectId, props.taskId] as const,
    () => {
      void hydratePopupContext();
    },
    { immediate: true },
  );

  watch(
    isBlockingLoading,
    (loading) => {
      clearContextLoadingTimer();
      if (!isPopup.value) {
        contextLoadingVisible.value = loading;
        return;
      }
      contextLoadingVisible.value = false;
      if (!loading) return;
      contextLoadingTimer = setTimeout(() => {
        if (disposed) return;
        contextLoadingVisible.value = isBlockingLoading.value;
        contextLoadingTimer = null;
      }, POPUP_CONTEXT_LOADING_NOTICE_MS);
    },
    { immediate: true },
  );

  watch(
    () => props.projectId,
    (projectId) => {
      if (projectId) void rememberPopupLastProject(projectId);
    },
    { immediate: true },
  );

  onUnmounted(() => {
    disposed = true;
    invalidateContextSnapshot("unmount");
    stopContextInvalidationListener();
    clearContextLoadingTimer();
  });

  return {
    isPopup,
    project,
    projectTask,
    orphan,
    orphanCwd,
    popupContextHydrating,
    popupContextHydrated,
    popupContentReady,
    contextLoadingVisible,
    conversationRouteState,
    hasContext,
    isContextLoading,
    isPopupContentLoading,
    isPopupPending,
    isBlockingLoading,
    shouldRenderChat,
    shouldShowContextLoading,
    emptyHeadline,
    contextSearchCwd,
    ensureOrphanCwd,
    ensureTaskReadyForMessage,
    captureContextSnapshot,
    isCurrentContextSnapshot,
    assertContextSnapshotCurrent,
    invalidateContextSnapshot,
    hydratePopupContext,
    hydrateMainContext,
    hydrateMainTaskRecord,
    prepareForRouteChange,
  };
}
