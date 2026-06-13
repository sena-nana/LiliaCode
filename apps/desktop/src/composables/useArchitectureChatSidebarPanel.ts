import { Network } from "lucide-vue-next";
import ArchitectureSidebarPanel from "../components/chat/ArchitectureSidebarPanel.vue";
import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerArchitectureChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "architecture",
    title: "架构图",
    icon: Network,
    order: 20,
    component: ArchitectureSidebarPanel,
  });
}
