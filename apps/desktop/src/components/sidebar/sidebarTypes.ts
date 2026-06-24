import type { SidebarConversationSummary } from "@lilia/contracts";

export interface UnifiedSidebarConversation extends SidebarConversationSummary {}

export interface SidebarRunningProcessItem {
  taskId: string;
  title: string;
  projectName?: string | null;
  route: string;
}
