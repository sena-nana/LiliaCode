import {
  createRouter,
  createWebHashHistory,
  createWebHistory,
  type RouterHistory,
} from "vue-router";
import { defineComponent, h } from "vue";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "./utils/perf";
import { createLazyLoadState } from "./utils/lazyLoadState";

function loadRouteComponent<T>(name: string, loader: () => Promise<T>): () => Promise<T> {
  const state = createLazyLoadState(() =>
    measurePerfAsync(
      "router.component.load",
      loader,
      { detail: name },
    )
  );
  return () => state.load();
}

const AppShell = loadRouteComponent(
  "AppShell",
  async () => (await import("./layouts/AppShell.vue")).default,
);
const PopupShell = loadRouteComponent(
  "PopupShell",
  async () => (await import("./layouts/PopupShell.vue")).default,
);
const ConversationStatusFloat = loadRouteComponent(
  "ConversationStatusFloat",
  async () => (await import("./pages/ConversationStatusFloat.vue")).default,
);
const loadTaskDetail = loadRouteComponent(
  "TaskDetail",
  async () => (await import("./pages/TaskDetail.vue")).default,
);
const TaskDetail = loadTaskDetail;
let taskDetailPreloadPromise: Promise<unknown> | null = null;
let taskDetailPreloadScheduled = false;
let cancelTaskDetailPreloadPaint: (() => void) | null = null;
let taskDetailPreloadIdleHandle: number | null = null;
const PopupDraftBoot = loadRouteComponent(
  "PopupDraftBoot",
  async () => (await import("./pages/PopupDraftBoot.vue")).default,
);
const Settings = loadRouteComponent(
  "Settings",
  async () => (await import("./pages/Settings.vue")).default,
);
const Automations = loadRouteComponent(
  "Automations",
  async () => (await import("./pages/Automations.vue")).default,
);
const ProjectsOverview = loadRouteComponent(
  "ProjectsOverview",
  async () => (await import("./pages/project/ProjectsOverview.vue")).default,
);
const ProjectShell = loadRouteComponent(
  "ProjectShell",
  async () => (await import("./pages/project/ProjectShell.vue")).default,
);
const SessionsView = loadRouteComponent(
  "SessionsView",
  async () => (await import("./pages/project/SessionsView.vue")).default,
);
const RoadmapView = loadRouteComponent(
  "RoadmapView",
  async () => (await import("./pages/project/RoadmapView.vue")).default,
);
const MemoryView = loadRouteComponent(
  "MemoryView",
  async () => (await import("./pages/project/MemoryView.vue")).default,
);

const Home = defineComponent({
  name: "LiliaHome",
  setup() {
    return () =>
      h(
        "section",
        { class: "empty-state" },
        h("p", null, "从左侧选择一个对话开始，或者新建一个对话。"),
      );
  },
});

export function shouldUsePopupHashHistory(hash: string): boolean {
  return hash.startsWith("#/popup");
}

function createDefaultHistory(): RouterHistory {
  if (
    typeof window !== "undefined" &&
    shouldUsePopupHashHistory(window.location.hash)
  ) {
    return createWebHashHistory();
  }
  return createWebHistory();
}

export function createLiliaRouter(history: RouterHistory = createDefaultHistory()) {
  const router = createRouter({
    history,
    routes: [
      {
        path: "/popup",
        component: PopupShell,
        children: [
          {
            path: "status",
            component: ConversationStatusFloat,
          },
          {
            path: "projects/:projectId/new",
            component: PopupDraftBoot,
            props: true,
          },
          {
            path: "projects/:projectId/tasks/:taskId",
            component: TaskDetail,
            props: (route) => ({
              projectId: route.params.projectId,
              taskId: route.params.taskId,
              variant: "popup",
            }),
          },
          {
            path: "chats/new",
            component: PopupDraftBoot,
          },
          {
            path: "chats/:taskId",
            component: TaskDetail,
            props: (route) => ({
              taskId: route.params.taskId,
              variant: "popup",
            }),
          },
        ],
      },
      {
        path: "/",
        component: AppShell,
        children: [
          { path: "", component: Home },
          { path: "projects", component: ProjectsOverview },
          // 项目主区：ViewTabs 在这里渲染；子路由互斥呈现 sessions / roadmap / memory。
          {
            path: "projects/:projectId",
            component: ProjectShell,
            props: true,
            children: [
              {
                path: "",
                name: "project-sessions",
                component: SessionsView,
                props: true,
                meta: { projectTab: "sessions" },
              },
              {
                path: "roadmap",
                name: "project-roadmap",
                component: RoadmapView,
                props: true,
                meta: { projectTab: "roadmap" },
              },
              {
                path: "memory",
                name: "project-memory",
                component: MemoryView,
                props: true,
                meta: { projectTab: "memory" },
              },
            ],
          },
          // 任务详情是 ProjectShell 的兄弟路由，进入聊天时 ViewTabs 不渲染。
          {
            path: "projects/:projectId/tasks/:taskId",
            component: TaskDetail,
            props: true,
          },
          {
            path: "chats/:taskId",
            component: TaskDetail,
            props: true,
          },
          { path: "settings", component: Settings },
          { path: "plugins", redirect: { path: "/settings", query: { tab: "plugins" } } },
          { path: "automations", component: Automations },
          {
            path: "import",
            redirect: (to) => ({
              path: "/settings",
              query: { ...to.query, tab: "import" },
            }),
          },
        ],
      },
      { path: "/:pathMatch(.*)*", redirect: "/" },
    ],
  });
  let navigationStage: ReturnType<typeof beginPerfStage> | null = null;
  router.beforeEach((to) => {
    navigationStage?.end("superseded");
    navigationStage = beginPerfStage("route.switch", { detail: to.fullPath });
  });
  router.afterEach(() => {
    navigationStage?.end("resolved");
    navigationStage = null;
  });
  return router;
}

export function preloadTaskDetailPage(): Promise<unknown> {
  if (!taskDetailPreloadPromise) {
    taskDetailPreloadPromise = loadTaskDetail().catch((err) => {
      taskDetailPreloadPromise = null;
      throw err;
    });
  }
  return taskDetailPreloadPromise;
}

export function scheduleTaskDetailPreload(detail = "intent"): void {
  if (taskDetailPreloadPromise || taskDetailPreloadScheduled) return;
  taskDetailPreloadScheduled = true;
  const cancelPaint = scheduleAfterPaint(() => {
    if (cancelTaskDetailPreloadPaint === cancelPaint) {
      cancelTaskDetailPreloadPaint = null;
    }
    if (taskDetailPreloadPromise) {
      taskDetailPreloadScheduled = false;
      return;
    }
    taskDetailPreloadIdleHandle = runWhenIdle(() => {
      taskDetailPreloadIdleHandle = null;
      taskDetailPreloadScheduled = false;
      void measurePerfAsync(
        "task-detail.preload.intent",
        () => preloadTaskDetailPage(),
        { detail },
      );
    });
  });
  cancelTaskDetailPreloadPaint = cancelPaint;
}

export function cancelTaskDetailPreloadSchedule(): void {
  cancelTaskDetailPreloadPaint?.();
  cancelTaskDetailPreloadPaint = null;
  if (taskDetailPreloadIdleHandle !== null) {
    cancelIdleRun(taskDetailPreloadIdleHandle);
    taskDetailPreloadIdleHandle = null;
  }
  taskDetailPreloadScheduled = false;
}

export const router = createLiliaRouter();

