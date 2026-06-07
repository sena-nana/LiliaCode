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
  icon: Component;
}

export const SETTINGS_TABS: SettingsTab[] = [
  {
    key: "appearance",
    label: "外观与窗口",
    icon: Palette,
  },
  {
    key: "providers",
    label: "连接",
    icon: Network,
  },
  {
    key: "agent",
    label: "Agent",
    icon: Bot,
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
  return SETTINGS_TABS.some((tab) => tab.key === candidate)
    ? (candidate as SettingsTabKey)
    : DEFAULT_SETTINGS_TAB;
}
