import {
  createRouter,
  createWebHistory,
  type RouterHistory,
} from "vue-router";
import { defineComponent, h } from "vue";
import AppShell from "./layouts/AppShell.vue";
import TaskDetail from "./pages/TaskDetail.vue";
import Settings from "./pages/Settings.vue";
import Plugins from "./pages/Plugins.vue";
import ProjectsOverview from "./pages/project/ProjectsOverview.vue";
import ProjectShell from "./pages/project/ProjectShell.vue";
import SessionsView from "./pages/project/SessionsView.vue";
import RoadmapView from "./pages/project/RoadmapView.vue";
import MemoryView from "./pages/project/MemoryView.vue";

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

export function createLiliaRouter(history: RouterHistory = createWebHistory()) {
  return createRouter({
    history,
    routes: [
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
              { path: "", component: SessionsView, props: true },
              { path: "roadmap", component: RoadmapView, props: true },
              { path: "memory", component: MemoryView, props: true },
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
          { path: "plugins", component: Plugins },
        ],
      },
      { path: "/:pathMatch(.*)*", redirect: "/" },
    ],
  });
}

export const router = createLiliaRouter();
