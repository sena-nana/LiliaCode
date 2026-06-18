import { registerArchitectureChatSidebarPanel } from "../../composables/useArchitectureChatSidebarPanel";
import { registerDebugChatSidebarPanel } from "../../composables/useDebugChatSidebarPanel";
import { registerIabChatSidebarPanel } from "../../composables/useIabChatSidebarPanel";

export function registerTaskDetailArchitectureSidebarPanel() {
  return registerArchitectureChatSidebarPanel();
}

export function registerTaskDetailDebugSidebarPanel() {
  return registerDebugChatSidebarPanel();
}

export function registerTaskDetailIabSidebarPanel() {
  return registerIabChatSidebarPanel();
}
