import { Globe } from "lucide-vue-next";
import IabSidebarPanel from "../components/chat/IabSidebarPanel.vue";
import { registerChatSidebarPanel } from "./useChatSidebar";

export function registerIabChatSidebarPanel(): () => void {
  return registerChatSidebarPanel({
    id: "iab",
    title: "IAB",
    icon: Globe,
    order: 10,
    component: IabSidebarPanel,
  });
}
