export type PluginScope = "user" | "project";

export type PluginBackendKind = "claude" | "codex";

export interface ClaudeSkill {
  scope: PluginScope;
  name: string;
  description: string;
  enabled: boolean;
  path: string;
}

export interface ClaudePlugin {
  scope: PluginScope;
  name: string;
  description: string;
  version: string;
  enabled: boolean;
  path: string;
}

export interface ClaudeMcpServer {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  envKeys: string[];
  enabled: boolean;
}

export interface ClaudeMcpServerInput {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  removeEnvKeys?: string[];
}

export interface CodexMcpServer {
  name: string;
  command: string;
  args: string[];
  envKeys: string[];
  enabled: boolean;
  transport: "stdio" | "http" | "oauth" | "unknown" | string;
  editable: boolean;
}

export interface CodexMcpServerInput {
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
  warnings: string[];
}

export interface CodexRuntimeExtensions {
  mcpServers: CodexMcpServer[];
  configPath: string | null;
  warnings: string[];
}

export interface AgentRuntimeExtensions {
  claude?: ClaudeRuntimeExtensions;
  codex?: CodexRuntimeExtensions;
}

export interface PluginsOverview {
  claudeUserSkills: ClaudeSkill[];
  claudeProjectSkills: ClaudeSkill[];
  claudeUserPlugins: ClaudePlugin[];
  claudeMcpServers: ClaudeMcpServer[];
  claudeMcpConfigPath: string | null;
  codexMcpServers: CodexMcpServer[];
  codexConfigPath: string | null;
  warnings: string[];
}
