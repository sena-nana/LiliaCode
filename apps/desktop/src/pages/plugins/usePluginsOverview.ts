import { computed, onMounted, ref } from "vue";
import {
  pluginsOverview,
  type ClaudeMcpServer,
  type ClaudePlugin,
  type ClaudeSkill,
  type CodexMcpServer,
  type PluginScope,
} from "../../services/plugins";
import { listProjects } from "../../services/projectsStore";

export type PluginsTab = "claude-skills" | "claude-plugins" | "claude-mcp" | "codex-mcp";

export function usePluginsOverview() {
  const tab = ref<PluginsTab>("claude-skills");
  const scope = ref<PluginScope>("user");
  const projects = computed(() => listProjects());
  const projectsWithCwd = computed(() =>
    projects.value.filter((p): p is typeof p & { cwd: string } => !!p.cwd),
  );
  const projectCwd = ref<string | null>(projectsWithCwd.value[0]?.cwd ?? null);
  const projectOptions = computed(() =>
    projectsWithCwd.value.map((p) => ({ value: p.cwd, label: p.name, hint: p.cwd })),
  );

  const userSkills = ref<ClaudeSkill[]>([]);
  const projectSkills = ref<ClaudeSkill[]>([]);
  const claudePlugins = ref<ClaudePlugin[]>([]);
  const claudeMcpServers = ref<ClaudeMcpServer[]>([]);
  const claudeMcpConfigPath = ref<string | null>(null);
  const codexServers = ref<CodexMcpServer[]>([]);
  const codexConfigPath = ref<string | null>(null);
  const warnings = ref<string[]>([]);
  const loading = ref(false);
  const errorText = ref<string | null>(null);

  const currentSkills = computed(() =>
    scope.value === "user" ? userSkills.value : projectSkills.value,
  );

  const currentScopeHint = computed(() =>
    scope.value === "user"
      ? "~/.claude/skills/"
      : projectCwd.value
        ? `${projectCwd.value}\\.claude\\skills\\`
        : "未选择项目",
  );

  async function refresh() {
    loading.value = true;
    errorText.value = null;
    try {
      const data = await pluginsOverview(projectCwd.value);
      userSkills.value = data.claudeUserSkills;
      projectSkills.value = data.claudeProjectSkills;
      claudePlugins.value = data.claudeUserPlugins;
      claudeMcpServers.value = data.claudeMcpServers;
      claudeMcpConfigPath.value = data.claudeMcpConfigPath;
      codexServers.value = data.codexMcpServers;
      codexConfigPath.value = data.codexConfigPath;
      warnings.value = data.warnings;
    } catch (err) {
      errorText.value = String(err);
    } finally {
      loading.value = false;
    }
  }

  function onProjectChange(cwd: string) {
    projectCwd.value = cwd;
    void refresh();
  }

  onMounted(() => refresh());

  return {
    tab,
    scope,
    projectCwd,
    projectOptions,
    userSkills,
    projectSkills,
    claudePlugins,
    claudeMcpServers,
    claudeMcpConfigPath,
    codexServers,
    codexConfigPath,
    warnings,
    loading,
    errorText,
    currentSkills,
    currentScopeHint,
    refresh,
    onProjectChange,
  };
}
