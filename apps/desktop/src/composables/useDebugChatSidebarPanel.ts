import DebugTimelinePanel from "../components/chat/DebugTimelinePanel.vue";
import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerDebugChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "debug",
    title: "Debug",
    order: 900,
    component: DebugTimelinePanel,
  });
}
