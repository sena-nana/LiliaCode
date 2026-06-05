import {
  Bot,
  FolderCog,
  Info,
  Network,
  Palette,
} from "lucide-vue-next";
import type { Component } from "vue";

export type SettingsTabKey =
  | "appearance"
  | "providers"
  | "agent"
  | "project"
  | "about";

export interface SettingsTab {
  key: SettingsTabKey;
  label: string;
  description: string;
  icon: Component;
}

export const SETTINGS_TABS: SettingsTab[] = [
  {
    key: "appearance",
    label: "外观与窗口",
    description: "主题、语言、弹出窗口",
    icon: Palette,
  },
  {
    key: "providers",
    label: "连接",
    description: "Provider、代理和 Assistant AI",
    icon: Network,
  },
  {
    key: "agent",
    label: "Agent",
    description: "Codex 默认行为",
    icon: Bot,
  },
  {
    key: "project",
    label: "项目",
    description: "项目偏好",
    icon: FolderCog,
  },
  {
    key: "about",
    label: "关于",
    description: "版本与说明",
    icon: Info,
  },
];

export const DEFAULT_SETTINGS_TAB: SettingsTabKey = "appearance";

export function normalizeSettingsTab(value: unknown): SettingsTabKey {
  const candidate = Array.isArray(value) ? value[0] : value;
  return SETTINGS_TABS.some((tab) => tab.key === candidate)
    ? (candidate as SettingsTabKey)
    : DEFAULT_SETTINGS_TAB;
}
