import { ref } from "vue";
import type { Router } from "vue-router";
import type { SidebarRunningProcessItem } from "../components/sidebar/sidebarTypes";
import { interruptTurn } from "../services/chat";
import { clearConversationRunning } from "./useConversationActivity";

export function useSidebarRunningProcesses(options: {
  router: Router;
  reportError: (message: string) => void;
}) {
  const stoppingTaskIds = ref<string[]>([]);

  function openRunningProcess(item: SidebarRunningProcessItem) {
    options.router.push(item.route);
  }

  async function stopRunningProcess(taskId: string) {
    if (stoppingTaskIds.value.includes(taskId)) return;
    stoppingTaskIds.value = [...stoppingTaskIds.value, taskId];
    try {
      await interruptTurn(taskId);
      clearConversationRunning(taskId);
    } catch (err) {
      options.reportError(`强行停止进程失败：${String(err)}`);
    } finally {
      stoppingTaskIds.value = stoppingTaskIds.value.filter((id) => id !== taskId);
    }
  }

  return {
    openRunningProcess,
    stoppingTaskIds,
    stopRunningProcess,
  };
}

