/**
 * 插件 / 技能服务：包装 plugins_* 系列 Tauri command。
 * Rust 侧字段命名走 camelCase（serde rename_all），前端不需 key 映射。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  HookDocumentUpdateInput,
  HookDocumentView,
  HooksOverview,
  HookSourceSummary,
  PluginBackendKind,
  PluginMcpServer,
  PluginMcpServerInput,
  PluginPackage,
  PluginScope,
  PluginSkill,
  PluginsOverview,
} from "@lilia/contracts";

export type {
  HookDocumentUpdateInput,
  HookDocumentView,
  HooksOverview,
  HookSourceSummary,
  PluginBackendKind,
  PluginMcpServer,
  PluginMcpServerInput,
  PluginPackage,
  PluginScope,
  PluginSkill,
  PluginsOverview,
};

export function pluginsOverview(projectCwd?: string | null): Promise<PluginsOverview> {
  return invoke<PluginsOverview>("plugins_overview", {
    projectCwd: projectCwd ?? null,
  });
}

export function hooksOverview(projectCwd?: string | null): Promise<HooksOverview> {
  return invoke<HooksOverview>("plugins_hooks_overview", {
    projectCwd: projectCwd ?? null,
  });
}

export function createSkill(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
  description: string,
): Promise<PluginSkill> {
  return invoke<PluginSkill>("plugins_create_skill", {
    scope,
    projectCwd,
    name,
    description,
  });
}

export function deleteSkill(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
): Promise<void> {
  return invoke<void>("plugins_delete_skill", {
    scope,
    projectCwd,
    name,
  });
}

export function setSkillEnabled(
  scope: PluginScope,
  projectCwd: string | null,
  name: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("plugins_set_skill_enabled", {
    scope,
    projectCwd,
    name,
    enabled,
  });
}

export function setPackageEnabled(
  backend: PluginBackendKind,
  scope: PluginScope,
  name: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("plugins_set_package_enabled", {
    backend,
    scope,
    name,
    enabled,
  });
}

export function createMcpServer(
  backend: PluginBackendKind,
  input: PluginMcpServerInput,
): Promise<PluginMcpServer> {
  return invoke<PluginMcpServer>("plugins_create_mcp_server", { backend, input });
}

export function updateMcpServer(
  backend: PluginBackendKind,
  name: string,
  input: PluginMcpServerInput,
): Promise<PluginMcpServer> {
  return invoke<PluginMcpServer>("plugins_update_mcp_server", { backend, name, input });
}

export function deleteMcpServer(backend: PluginBackendKind, name: string): Promise<void> {
  return invoke<void>("plugins_delete_mcp_server", { backend, name });
}

export function setMcpServerEnabled(
  backend: PluginBackendKind,
  name: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("plugins_set_mcp_server_enabled", { backend, name, enabled });
}

export function openMcpConfig(backend: PluginBackendKind): Promise<void> {
  return invoke<void>("plugins_open_mcp_config", { backend });
}

export function readHookSource(source: HookSourceSummary): Promise<HookDocumentView> {
  return invoke<HookDocumentView>("plugins_read_hook_source", { source });
}

export function updateHookSource(
  source: HookSourceSummary,
  input: HookDocumentUpdateInput,
): Promise<HookDocumentView> {
  return invoke<HookDocumentView>("plugins_update_hook_source", { source, input });
}

export function createHookSource(
  backend: PluginBackendKind,
  scope: string,
  projectCwd?: string | null,
): Promise<HookSourceSummary> {
  return invoke<HookSourceSummary>("plugins_create_hook_source", {
    backend,
    scope,
    projectCwd: projectCwd ?? null,
  });
}

export function deleteHookSource(source: HookSourceSummary): Promise<void> {
  return invoke<void>("plugins_delete_hook_source", { source });
}

export function setHookSourceEnabled(
  source: HookSourceSummary,
  enabled: boolean,
): Promise<HookSourceSummary> {
  return invoke<HookSourceSummary>("plugins_set_hook_source_enabled", {
    source,
    enabled,
  });
}

export function openHookConfig(source: HookSourceSummary): Promise<void> {
  return invoke<void>("plugins_open_hook_config", { source });
}
