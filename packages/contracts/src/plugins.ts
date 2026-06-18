export type PluginScope = "user" | "project";

export type PluginBackendKind = "claude" | "codex";

export interface PluginSkill {
  backend: PluginBackendKind;
  scope: PluginScope;
  name: string;
  description: string;
  enabled: boolean;
  path: string;
}

export interface PluginPackage {
  backend: PluginBackendKind;
  scope: PluginScope;
  name: string;
  description: string;
  version: string;
  enabled: boolean;
  path: string;
}

export interface PluginMcpServer {
  backend: PluginBackendKind;
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  envKeys: string[];
  enabled: boolean;
  editable: boolean;
  transport?: "stdio" | "http" | "oauth" | "unknown" | string;
}

export interface PluginMcpServerInput {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  removeEnvKeys?: string[];
}

export interface ClaudeRuntimePlugin {
  type: "local";
  path: string;
}

export interface ClaudeRuntimeMcpServer {
  type: "stdio";
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export interface ClaudeRuntimeExtensions {
  skills: string[];
  plugins: ClaudeRuntimePlugin[];
  mcpServers: Record<string, ClaudeRuntimeMcpServer>;
  hooks?: Record<string, unknown>;
  warnings: string[];
}

export interface CodexRuntimeExtensions {
  mcpServers: PluginMcpServer[];
  configPath: string | null;
  warnings: string[];
}

export interface AgentRuntimeExtensions {
  claude?: ClaudeRuntimeExtensions;
  codex?: CodexRuntimeExtensions;
}

export interface PluginsOverview {
  skills: PluginSkill[];
  packages: PluginPackage[];
  mcpServers: PluginMcpServer[];
  configPaths: Partial<Record<PluginBackendKind, string | null>>;
  warnings: string[];
}
