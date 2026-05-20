import {
  createRouter,
  createWebHistory,
  type RouterHistory,
} from "vue-router";
import AppShell from "./layouts/AppShell.vue";
import Projects from "./pages/Projects.vue";
import ProjectDetail from "./pages/ProjectDetail.vue";
import TaskDetail from "./pages/TaskDetail.vue";
import Settings from "./pages/Settings.vue";

export function createLiliaRouter(history: RouterHistory = createWebHistory()) {
  return createRouter({
    history,
    routes: [
      {
        path: "/",
        component: AppShell,
        children: [
          { path: "", redirect: "/projects" },
          { path: "projects", component: Projects },
          {
            path: "projects/:projectId",
            component: ProjectDetail,
            props: true,
          },
          {
            path: "projects/:projectId/tasks/:taskId",
            component: TaskDetail,
            props: true,
          },
          { path: "settings", component: Settings },
        ],
      },
      { path: "/:pathMatch(.*)*", redirect: "/projects" },
    ],
  });
}

export const router = createLiliaRouter();
