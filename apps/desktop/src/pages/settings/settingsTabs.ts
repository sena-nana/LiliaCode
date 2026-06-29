import {
  Bot,
  Brain,
  Download,
  FolderCog,
  Gauge,
  Info,
  MonitorSmartphone,
  Network,
  PanelTop,
  Palette,
  Puzzle,
  Server,
  Sparkles,
  Workflow,
} from "@lucide/vue";
import type { Component } from "vue";

export type SettingsTabKey =
  | "appearance"
  | "window"
  | "providers"
  | "remote-control"
  | "assistant"
  | "model-config"
  | "agent"
  | "quota"
  | "plugin-skills"
  | "plugin-packages"
  | "plugin-hooks"
  | "plugin-mcp"
  | "import"
  | "project"
  | "about";

export interface SettingsTab {
  key: SettingsTabKey;
  label: string;
  icon: Component;
}

export const SETTINGS_TABS: SettingsTab[] = [
  {
    key: "appearance",
    label: "外观",
    icon: Palette,
  },
  {
    key: "window",
    label: "窗口",
    icon: PanelTop,
  },
  {
    key: "providers",
    label: "连接",
    icon: Network,
  },
  {
    key: "remote-control",
    label: "Android 远控",
    icon: MonitorSmartphone,
  },
  {
    key: "assistant",
    label: "Provider 配置",
    icon: Sparkles,
  },
  {
    key: "model-config",
    label: "模型配置",
    icon: Brain,
  },
  {
    key: "agent",
    label: "Agent",
    icon: Bot,
  },
  {
    key: "quota",
    label: "额度",
    icon: Gauge,
  },
  {
    key: "plugin-skills",
    label: "技能",
    icon: Sparkles,
  },
  {
    key: "plugin-packages",
    label: "插件",
    icon: Puzzle,
  },
  {
    key: "plugin-hooks",
    label: "Hooks",
    icon: Workflow,
  },
  {
    key: "plugin-mcp",
    label: "MCP",
    icon: Server,
  },
  {
    key: "import",
    label: "导入对话",
    icon: Download,
  },
  {
    key: "project",
    label: "项目",
    icon: FolderCog,
  },
  {
    key: "about",
    label: "关于",
    icon: Info,
  },
];

export const DEFAULT_SETTINGS_TAB: SettingsTabKey = "appearance";

export function normalizeSettingsTab(value: unknown): SettingsTabKey {
  const candidate = Array.isArray(value) ? value[0] : value;
  if (candidate === "plugins") return "plugin-skills";
  return SETTINGS_TABS.some((tab) => tab.key === candidate)
    ? (candidate as SettingsTabKey)
    : DEFAULT_SETTINGS_TAB;
}

