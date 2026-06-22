import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  hooksOverview,
  type HookSourceSummary,
  pluginsOverview,
  type PluginMcpServer,
  type PluginPackage,
  type PluginSkill,
} from "../../services/plugins";
import { listProjects } from "../../services/projectsStore";

export type PluginsTab =
  | "claude-skills"
  | "claude-plugins"
  | "claude-hooks"
  | "claude-mcp"
  | "codex-hooks"
  | "codex-mcp";

export function usePluginsOverview(options: {
  isDisposed?: () => boolean;
} = {}) {
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
  const claudeHookSources = ref<HookSourceSummary[]>([]);
  const codexServers = ref<PluginMcpServer[]>([]);
  const codexConfigPath = ref<string | null>(null);
  const codexHookSources = ref<HookSourceSummary[]>([]);
  const warnings = ref<string[]>([]);
  const loading = ref(false);
  const errorText = ref<string | null>(null);
  let refreshSeq = 0;
  let disposed = false;
  const isDisposed = () => disposed || options.isDisposed?.() === true;

  async function refresh() {
    if (isDisposed()) return;
    const seq = ++refreshSeq;
    loading.value = true;
    errorText.value = null;
    try {
      const [pluginsData, hooksData] = await Promise.all([
        pluginsOverview(projectCwd.value),
        hooksOverview(projectCwd.value),
      ]);
      if (isDisposed() || seq !== refreshSeq) return;
      userSkills.value = pluginsData.skills.filter(
        (skill) => skill.backend === "claude" && skill.scope === "user",
      );
      projectSkills.value = pluginsData.skills.filter(
        (skill) => skill.backend === "claude" && skill.scope === "project",
      );
      claudePlugins.value = pluginsData.packages.filter((plugin) => plugin.backend === "claude");
      claudeMcpServers.value = pluginsData.mcpServers.filter((server) => server.backend === "claude");
      claudeMcpConfigPath.value = pluginsData.configPaths.claude ?? null;
      claudeHookSources.value = hooksData.sources.filter((source) => source.backend === "claude");
      codexServers.value = pluginsData.mcpServers.filter((server) => server.backend === "codex");
      codexConfigPath.value = pluginsData.configPaths.codex ?? null;
      codexHookSources.value = hooksData.sources.filter((source) => source.backend === "codex");
      warnings.value = [...pluginsData.warnings, ...hooksData.warnings];
    } catch (err) {
      if (!isDisposed() && seq === refreshSeq) errorText.value = String(err);
    } finally {
      if (!isDisposed() && seq === refreshSeq) loading.value = false;
    }
  }

  onMounted(() => refresh());
  watch(projectCwd, () => {
    void refresh();
  });

  onBeforeUnmount(() => {
    disposed = true;
    refreshSeq += 1;
  });

  return {
    tab,
    projectCwd,
    projectOptions,
    userSkills,
    projectSkills,
    claudePlugins,
    claudeMcpServers,
    claudeMcpConfigPath,
    claudeHookSources,
    codexServers,
    codexConfigPath,
    codexHookSources,
    warnings,
    loading,
    errorText,
    refresh,
  };
}
