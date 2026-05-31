/**
 * 插件 / 技能服务：包装 plugins_* 系列 Tauri command。
 * Rust 侧字段命名走 camelCase（serde rename_all），前端不需 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  ClaudePlugin,
  ClaudeSkill,
  CodexMcpServer,
  PluginScope,
  PluginsOverview,
} from "@lilia/contracts";

export type { ClaudePlugin, ClaudeSkill, CodexMcpServer, PluginScope, PluginsOverview };

export function pluginsOverview(projectCwd?: string | null): Promise<PluginsOverview> {
  return invoke<PluginsOverview>("plugins_overview", {
    projectCwd: projectCwd ?? null,
  });
}

export function listClaudeSkills(
  scope: PluginScope,
  projectCwd?: string | null,
): Promise<ClaudeSkill[]> {
  return invoke<ClaudeSkill[]>("plugins_list_claude_skills", {
    scope,
    projectCwd: projectCwd ?? null,
  });
}

export function createClaudeSkill(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
  description: string,
): Promise<ClaudeSkill> {
  return invoke<ClaudeSkill>("plugins_create_claude_skill", {
    scope,
    projectCwd,
    name,
    description,
  });
}

export function deleteClaudeSkill(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
): Promise<void> {
  return invoke<void>("plugins_delete_claude_skill", {
    scope,
    projectCwd,
    name,
  });
}

export function setClaudeSkillEnabled(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("plugins_set_claude_skill_enabled", {
    scope,
    projectCwd,
    name,
    enabled,
  });
}

export function setClaudePluginEnabled(
  scope: PluginScope,
  name: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("plugins_set_claude_plugin_enabled", {
    scope,
    name,
    enabled,
  });
}

export function listClaudePlugins(scope: PluginScope): Promise<ClaudePlugin[]> {
  return invoke<ClaudePlugin[]>("plugins_list_claude_plugins", { scope });
}

export function listCodexMcpServers(): Promise<CodexMcpServer[]> {
  return invoke<CodexMcpServer[]>("plugins_list_codex_mcp_servers");
}

export function openCodexConfig(): Promise<void> {
  return invoke<void>("plugins_open_codex_config");
}
