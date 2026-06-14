import { computed, onMounted, ref } from "vue";
import {
  pluginsOverview,
  type PluginMcpServer,
  type PluginPackage,
  type PluginSkill,
} from "../../services/plugins";
import { listProjects } from "../../services/projectsStore";

export type PluginsTab = "claude-skills" | "claude-plugins" | "claude-mcp" | "codex-mcp";

export function usePluginsOverview() {
  const tab = ref<PluginsTab>("claude-skills");
  const projects = computed(() => listProjects());
  const projectsWithCwd = computed(() =>
    projects.value.filter((p): p is typeof p & { cwd: string } => !!p.cwd),
  );
  const projectCwd = ref<string | null>(projectsWithCwd.value[0]?.cwd ?? null);
  const projectOptions = computed(() =>
    projectsWithCwd.value.map((p) => ({ value: p.cwd, label: p.name, hint: p.cwd })),
  );

  const userSkills = ref<PluginSkill[]>([]);
  const projectSkills = ref<PluginSkill[]>([]);
  const claudePlugins = ref<PluginPackage[]>([]);
  const claudeMcpServers = ref<PluginMcpServer[]>([]);
  const claudeMcpConfigPath = ref<string | null>(null);
  const codexServers = ref<PluginMcpServer[]>([]);
  const codexConfigPath = ref<string | null>(null);
  const warnings = ref<string[]>([]);
  const loading = ref(false);
  const errorText = ref<string | null>(null);

  async function refresh() {
    loading.value = true;
    errorText.value = null;
    try {
      const data = await pluginsOverview(projectCwd.value);
      userSkills.value = data.skills.filter(
        (skill) => skill.backend === "claude" && skill.scope === "user",
      );
      projectSkills.value = data.skills.filter(
        (skill) => skill.backend === "claude" && skill.scope === "project",
      );
      claudePlugins.value = data.packages.filter((plugin) => plugin.backend === "claude");
      claudeMcpServers.value = data.mcpServers.filter((server) => server.backend === "claude");
      claudeMcpConfigPath.value = data.configPaths.claude ?? null;
      codexServers.value = data.mcpServers.filter((server) => server.backend === "codex");
      codexConfigPath.value = data.configPaths.codex ?? null;
      warnings.value = data.warnings;
    } catch (err) {
      errorText.value = String(err);
    } finally {
      loading.value = false;
    }
  }

  onMounted(() => refresh());

  return {
    tab,
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
    refresh,
  };
}
