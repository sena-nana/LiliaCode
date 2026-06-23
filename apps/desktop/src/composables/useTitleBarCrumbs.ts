import type { SidebarConversationSummary } from "@lilia/contracts";
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";
import {
  areSidebarConversationsLoaded,
  ensureSidebarConversationsLoaded,
  findSidebarConversation,
} from "../services/sidebarConversations";
import {
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

const titlebarTasksStoreLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "titlebar.crumbs.tasks-store.load",
    async () => await import("../services/tasksStore"),
  )
);
const titlebarProjectsStoreLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "titlebar.crumbs.projects-store.load",
    async () => await import("../services/projectsStore"),
  )
);

export interface TitleBarCrumb {
  text: string;
  muted?: boolean;
}

interface FallbackStores {
  getOrphanConversation: (taskId: string) => { title: string } | undefined;
  getProject?: (projectId: string) => { name: string } | undefined;
  getTask: (projectId: string, taskId: string) => { title: string } | undefined;
}

function paramAsString(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) return value[0];
  return value;
}

function isProjectDraftId(taskId: string): boolean {
  return taskId.startsWith("t-draft-");
}

function isOrphanDraftId(taskId: string): boolean {
  return taskId.startsWith("o-draft-");
}

function findConversationSummary(
  projectId: string | undefined,
  taskId: string | undefined,
): SidebarConversationSummary | null {
  if (!taskId) return null;
  return findSidebarConversation(taskId, projectId ?? null);
}

export function useTitleBarCrumbs() {
  const route = useRoute();
  const projectId = computed(() => paramAsString(route.params.projectId));
  const taskId = computed(() => paramAsString(route.params.taskId));
  const fallbackStores = shallowRef<FallbackStores | null>(null);
  let fallbackIdleHandle: number | null = null;
  let cancelFallbackPaint: (() => void) | null = null;
  let fallbackSeq = 0;
  let disposed = false;

  const summary = computed(() => findConversationSummary(projectId.value, taskId.value));

  watch(
    () => [route.path, projectId.value, taskId.value, summary.value?.taskId] as const,
    ([path, projectIdValue, taskIdValue, summaryTaskId]) => {
      fallbackSeq += 1;
      cancelFallbackPaint?.();
      cancelFallbackPaint = null;
      if (fallbackIdleHandle !== null) {
        cancelIdleRun(fallbackIdleHandle);
        fallbackIdleHandle = null;
      }
      if (!taskIdValue) return;
      const isProjectConversationRoute = !!projectIdValue;
      const isOrphanConversationRoute = path.startsWith("/chats/");
      if (!isProjectConversationRoute && !isOrphanConversationRoute) return;
      if (summaryTaskId) {
        fallbackStores.value = null;
        return;
      }

      void ensureSidebarConversationsLoaded().catch((err) => {
        console.error("[titlebar] ensure sidebar conversations failed", err);
      });

      if (!areSidebarConversationsLoaded()) {
        fallbackStores.value = null;
      }

      const seq = fallbackSeq;
      cancelFallbackPaint = scheduleAfterPaint(() => {
        cancelFallbackPaint = null;
        if (seq !== fallbackSeq || findConversationSummary(projectIdValue, taskIdValue)) return;
        fallbackIdleHandle = runWhenIdle(() => {
          fallbackIdleHandle = null;
          if (seq !== fallbackSeq || findConversationSummary(projectIdValue, taskIdValue)) return;
          void measurePerfAsync(
            "titlebar.crumbs.fallback",
            async () => {
              const [{ getOrphanConversation, getTask }, projectsStore] = await Promise.all([
                titlebarTasksStoreLoad.load(),
                projectIdValue ? titlebarProjectsStoreLoad.load() : Promise.resolve(null),
              ]);
              if (
                disposed ||
                seq !== fallbackSeq ||
                findConversationSummary(projectIdValue, taskIdValue)
              ) {
                return;
              }
              fallbackStores.value = {
                getOrphanConversation,
                getProject: projectsStore?.getProject,
                getTask,
              };
            },
            { detail: `${projectIdValue ?? "orphan"}:${taskIdValue}` },
          ).catch((err) => {
            console.error("[titlebar] resolve breadcrumbs failed", err);
          });
        });
      });
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    disposed = true;
    fallbackSeq += 1;
    cancelFallbackPaint?.();
    cancelFallbackPaint = null;
    if (fallbackIdleHandle !== null) {
      cancelIdleRun(fallbackIdleHandle);
      fallbackIdleHandle = null;
    }
  });

  const crumbs = computed<TitleBarCrumb[]>(() => {
    const path = route.path;
    const projectIdValue = projectId.value;
    const taskIdValue = taskId.value;
    const conversation = summary.value;
    const stores = fallbackStores.value;

    if (projectIdValue && taskIdValue) {
      return [
        {
          text: conversation?.projectName ||
            stores?.getProject?.(projectIdValue)?.name ||
            "未知项目",
          muted: true,
        },
        {
          text: conversation?.title ||
            stores?.getTask(projectIdValue, taskIdValue)?.title ||
            (isProjectDraftId(taskIdValue) ? "新对话" : "未知任务"),
        },
      ];
    }

    if (path.startsWith("/chats/") && taskIdValue) {
      return [
        { text: "收集箱", muted: true },
        {
          text: conversation?.title ||
            stores?.getOrphanConversation(taskIdValue)?.title ||
            (isOrphanDraftId(taskIdValue) ? "新对话" : "未知任务"),
        },
      ];
    }

    if (path === "/settings") return [{ text: "设置" }];
    if (path === "/automations") return [{ text: "自动化" }];

    return [{ text: "Lilia" }];
  });

  return {
    projectId,
    taskId,
    crumbs,
  };
}
