import {
  createRouter,
  createWebHistory,
  type RouterHistory,
} from "vue-router";
import { defineComponent, h } from "vue";
import AppShell from "./layouts/AppShell.vue";
import TaskDetail from "./pages/TaskDetail.vue";
import Settings from "./pages/Settings.vue";

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
          {
            path: "projects/:projectId/tasks/:taskId",
            component: TaskDetail,
            props: true,
          },
          { path: "settings", component: Settings },
        ],
      },
      { path: "/:pathMatch(.*)*", redirect: "/" },
    ],
  });
}

export const router = createLiliaRouter();
