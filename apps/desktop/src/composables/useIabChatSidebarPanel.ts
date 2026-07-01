import Globe from "@lucide/vue/dist/esm/icons/globe.mjs";
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

