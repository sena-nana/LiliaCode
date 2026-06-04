import { computed, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { homeDir } from "@tauri-apps/api/path";
import {
  ensureProjectTasksLoaded,
  ensureTaskLoaded,
  getOrphanConversation,
  getTask,
  promoteDraftOrphan,
  promoteDraftTask,
  resolveConversationRouteState,
} from "../../services/tasksStore";
import { ensureProjectLoaded, getProject } from "../../services/projectsStore";
import { rememberPopupLastProject } from "../../services/popupWindows";
import type { ChatAttachment } from "@lilia/contracts";

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

  const conversationRouteState = computed(() =>
    resolveConversationRouteState(props.projectId, props.taskId),
  );
  const hasContext = computed(() => {
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

  async function ensureOrphanCwd(): Promise<string> {
    if (orphanCwd.value) return orphanCwd.value;
    try {
      orphanCwd.value = await homeDir();
    } catch {
      orphanCwd.value = "";
    }
    return orphanCwd.value;
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
    const routeState = resolveConversationRouteState(props.projectId, props.taskId);
    if (props.projectId && routeState.isDraftRoute) {
      if (!routeState.isLiveDraft) throw new Error("草稿已失效，请重新创建对话");
      await promoteDraftTask(props.taskId, titleForMessage(content, outgoingAttachments));
    } else if (!props.projectId && routeState.isDraftRoute) {
      if (!routeState.isLiveDraft) throw new Error("草稿已失效，请重新创建对话");
      await promoteDraftOrphan(props.taskId, titleForMessage(content, outgoingAttachments));
    }
  }

  function popupNewDraftRoute(projectId: string | undefined): string {
    return projectId ? `/popup/projects/${projectId}/new` : "/popup/chats/new";
  }

  function clearContextLoadingTimer() {
    if (contextLoadingTimer === null) return;
    clearTimeout(contextLoadingTimer);
    contextLoadingTimer = null;
  }

  async function hydratePopupContext() {
    if (!isPopup.value) {
      popupContextHydrating.value = false;
      popupContextHydrated.value = true;
      return;
    }

    const seq = ++popupContextSeq;
    const projectId = props.projectId;
    const taskId = props.taskId;
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
      if (seq === popupContextSeq && taskId === props.taskId && projectId === props.projectId) {
        popupContextHydrating.value = false;
        popupContextHydrated.value = true;
      }
    }
  }

  async function hydrateMainContext() {
    if (isPopup.value) return;
    try {
      if (props.projectId) {
        await Promise.all([
          ensureProjectLoaded(props.projectId),
          ensureProjectTasksLoaded(props.projectId),
          ensureTaskLoaded(props.taskId, props.projectId),
        ]);
      } else {
        await ensureTaskLoaded(props.taskId, null);
      }
    } catch (err) {
      console.error("[chat] hydrate context failed", err);
    }
  }

  function prepareForRouteChange() {
    popupContentReady.value = !isPopup.value || conversationRouteState.value.isLiveDraft;
  }

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

  watch(
    () => [props.variant, props.projectId, props.taskId, popupContextHydrated.value] as const,
    ([variant, projectId, _taskId, contextHydrated]) => {
      if (variant !== "popup") return;
      if (contextHydrated && conversationRouteState.value.isLostDraft) {
        void router.replace(popupNewDraftRoute(projectId));
      }
    },
    { immediate: true },
  );

  onUnmounted(clearContextLoadingTimer);

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
    hydratePopupContext,
    hydrateMainContext,
    prepareForRouteChange,
  };
}
