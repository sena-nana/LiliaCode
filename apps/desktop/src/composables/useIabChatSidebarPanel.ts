import { Globe } from "lucide-vue-next";
import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerIabChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "iab",
    title: "IAB",
    icon: Globe,
    order: 10,
    loader: async () => (await import("../components/chat/IabSidebarPanel.vue")).default,
  });
}
