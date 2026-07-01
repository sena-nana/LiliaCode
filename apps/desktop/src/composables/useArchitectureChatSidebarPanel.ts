import Network from "@lucide/vue/dist/esm/icons/network.mjs";
import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerArchitectureChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "architecture",
    title: "架构图",
    icon: Network,
    order: 20,
    loader: async () => (await import("../components/chat/ArchitectureSidebarPanel.vue")).default,
  });
}

