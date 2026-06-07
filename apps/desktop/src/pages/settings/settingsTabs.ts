import {
  Bot,
  FolderCog,
  Info,
  Network,
  PanelTop,
  Palette,
  Sparkles,
} from "lucide-vue-next";
import type { Component } from "vue";

export type SettingsTabKey =
  | "appearance"
  | "window"
  | "providers"
  | "assistant"
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
    key: "assistant",
    label: "辅助能力",
    icon: Sparkles,
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
