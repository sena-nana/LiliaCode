import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerDebugChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "debug",
    title: "Debug",
    order: 900,
    loader: async () => (await import("../components/chat/DebugTimelinePanel.vue")).default,
  });
}

