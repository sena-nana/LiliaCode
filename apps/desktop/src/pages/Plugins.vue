<script setup lang="ts">
import "../styles/pages/plugins.css";
import { computed, defineAsyncComponent, onBeforeUnmount, ref, watch, type Component } from "vue";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Loader2,
  Pencil,
  Plus,
  Search,
  Trash2,
} from "@lucide/vue";
import {
  hookScopeLabel,
  hookSourceEditLabel,
  hookSourceFormatLabel,
  hookSourceStateLabel,
  hookTrustStateLabel,
  pluginEnabledLabel,
  pluginMcpBackendLabel,
  pluginMcpTransportLabel,
  pluginToggleActionLabel,
} from "@lilia/contracts";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import {
  createHookSource,
  createSkill,
  deleteHookSource,
  deleteMcpServer,
  deleteSkill,
  openHookConfig,
  openMcpConfig,
  readHookSource,
  setMcpServerEnabled,
  setPackageEnabled,
  setSkillEnabled,
  type HookDocumentView,
  type HookHandlerView,
  type HookSourceSummary,
  type PluginMcpServer,
  type PluginPackage,
  type PluginSkill,
} from "../services/plugins";
import { measurePerfAsync } from "../utils/perf";
import { usePluginsOverview } from "./plugins/usePluginsOverview";
import { useHookSourceEditor } from "./plugins/useHookSourceEditor";
import { useMcpServerEditor } from "./plugins/useMcpServerEditor";
import PluginsTabBar from "./plugins/PluginsTabBar.vue";
import { createLazyLoadState } from "../utils/lazyLoadState";

const skillCreateDialogLoad = createLazyLoadState<Component>(() =>
  measurePerfAsync(
    "plugins.skill-create-dialog.load",
    async () => (await import("./plugins/SkillCreateDialog.vue")).default,
  )
);
const hookSourceEditorDialogLoad = createLazyLoadState<Component>(() =>
  measurePerfAsync(
    "plugins.hook-editor-dialog.load",
    async () => (await import("./plugins/HookSourceEditorDialog.vue")).default,
  )
);
const mcpServerEditorDialogLoad = createLazyLoadState<Component>(() =>
  measurePerfAsync(
    "plugins.mcp-editor-dialog.load",
    async () => (await import("./plugins/McpServerEditorDialog.vue")).default,
  )
);

async function loadSkillCreateDialog(): Promise<Component> {
  return skillCreateDialogLoad.load();
}

async function loadHookSourceEditorDialog(): Promise<Component> {
  return hookSourceEditorDialogLoad.load();
}

async function loadMcpServerEditorDialog(): Promise<Component> {
  return mcpServerEditorDialogLoad.load();
}

const SkillCreateDialog = defineAsyncComponent({
  suspensible: false,
  loader: loadSkillCreateDialog,
});

const HookSourceEditorDialog = defineAsyncComponent({
  suspensible: false,
  loader: loadHookSourceEditorDialog,
});

const McpServerEditorDialog = defineAsyncComponent({
  suspensible: false,
  loader: loadMcpServerEditorDialog,
});

const {
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
} = usePluginsOverview({
  isDisposed: () => disposed,
});

const showCreate = ref(false);
const newName = ref("");
const newDesc = ref("");
const creating = ref(false);
const createError = ref<string | null>(null);
const pendingRemoveSkill = ref<PluginSkill | null>(null);
const pendingRemoveMcp = ref<PluginMcpServer | null>(null);
const pendingRemoveHook = ref<HookSourceSummary | null>(null);
const removing = ref(false);
const query = ref("");
const selectedKey = ref<string | null>(null);
const selectedHookDocument = ref<HookDocumentView | null>(null);
const hookDocumentLoading = ref(false);
const hookDocumentError = ref<string | null>(null);
let hookDocumentRequestId = 0;
let disposed = false;

const mcpEditor = useMcpServerEditor<PluginMcpServer>({
  backend: "claude",
  label: "Claude MCP",
  refresh,
  isDisposed: () => disposed,
});
const codexMcpEditor = useMcpServerEditor<PluginMcpServer>({
  backend: "codex",
  label: "Codex MCP",
  refresh,
  isDisposed: () => disposed,
});
const hookEditor = useHookSourceEditor({
  refresh,
  isDisposed: () => disposed,
  onSaved: (document) => {
    if (disposed) return;
    selectedHookDocument.value = document;
    hookDocumentError.value = null;
  },
});
const USER_SKILLS_ROOT = "~/.claude/skills/";

type PluginEntry =
  | { kind: "skill"; key: string; title: string; meta: string; searchText: string; item: PluginSkill }
  | { kind: "plugin"; key: string; title: string; meta: string; searchText: string; item: PluginPackage }
  | {
      kind: "claude-mcp";
      key: string;
      title: string;
      meta: string;
      searchText: string;
      item: PluginMcpServer;
    }
  | {
      kind: "codex-mcp";
      key: string;
      title: string;
      meta: string;
      searchText: string;
      item: PluginMcpServer;
    }
  | {
      kind: "claude-hook";
      key: string;
      title: string;
      meta: string;
      searchText: string;
      item: HookSourceSummary;
    }
  | {
      kind: "codex-hook";
      key: string;
      title: string;
      meta: string;
      searchText: string;
      item: HookSourceSummary;
    };

interface DetailRow {
  label: string;
  value: string;
  code?: boolean;
}

type HookEntry = Extract<PluginEntry, { kind: "claude-hook" | "codex-hook" }>;

function commandLine(command: string, args: string[]) {
  return [command, ...args].filter(Boolean).join(" ").trim();
}

function codexServerSummary(server: PluginMcpServer) {
  return commandLine(server.command, server.args) || `${pluginMcpTransportLabel(server.transport)} MCP server`;
}

function normalizePath(value: string | null | undefined) {
  return (value ?? "").replace(/\\/g, "/").toLocaleLowerCase();
}

function projectFolderFromSkillPath(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const marker = "/.claude/skills/";
  const markerIndex = normalized.toLocaleLowerCase().indexOf(marker);
  if (markerIndex <= 0) return "";
  const projectPath = normalized.slice(0, markerIndex).replace(/\/*$/, "");
  const parts = projectPath.split("/");
  return parts[parts.length - 1] ?? "";
}

function projectSkillsRootFromPath(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const marker = "/.claude/skills/";
  const markerIndex = normalized.toLocaleLowerCase().indexOf(marker);
  if (markerIndex <= 0) return "";
  return `${normalized.slice(0, markerIndex)}\\.claude\\skills\\`;
}

function projectLabelForSkill(skill: PluginSkill) {
  if (skill.scope !== "project") return "";
  const skillPath = normalizePath(skill.path);
  const matchedProject = projectOptions.value.find((project) =>
    skillPath.startsWith(normalizePath(project.value)),
  );
  if (matchedProject) return matchedProject.label;
  return projectFolderFromSkillPath(skill.path) || "项目";
}

function skillLocation(skill: PluginSkill) {
  if (skill.scope === "user") return USER_SKILLS_ROOT;
  const skillPath = normalizePath(skill.path);
  const project = projectOptions.value.find((option) =>
    skillPath.startsWith(normalizePath(option.value)),
  );
  return project
    ? `${project.value}\\.claude\\skills\\`
    : projectSkillsRootFromPath(skill.path) || "未选择项目";
}

function hookSourceMeta(source: HookSourceSummary) {
  return [
    hookScopeLabel(source.scope),
    hookSourceEditLabel(source),
    hookSourceStateLabel(source),
    source.managed ? "Managed" : "",
    source.warnings.length ? `${source.warnings.length} 警告` : "",
  ].filter(Boolean).join(" · ");
}

function joinSearchText(...parts: Array<string | null | undefined>) {
  return parts.filter(Boolean).join("\n");
}

function buildHookEntry(kind: HookEntry["kind"], source: HookSourceSummary): HookEntry {
  return {
    kind,
    key: `${kind}:${source.id}`,
    title: source.name,
    meta: hookSourceMeta(source),
    searchText: joinSearchText(
      source.name,
      source.path,
      source.description,
      source.warnings.join("\n"),
      source.limitations.join("\n"),
    ),
    item: source,
  };
}

function isHookEntry(
  entry: PluginEntry | null,
): entry is HookEntry {
  return !!entry && (entry.kind === "claude-hook" || entry.kind === "codex-hook");
}

function hookDocumentFor(source: HookSourceSummary | null) {
  return source && selectedHookDocument.value?.source.id === source.id
    ? selectedHookDocument.value
    : null;
}

function mergeUniqueTexts(...groups: string[][]) {
  return Array.from(new Set(groups.flat()));
}

const allEntries = computed<PluginEntry[]>(() => {
  if (tab.value === "claude-skills") {
    return [...userSkills.value, ...projectSkills.value].map((skill) => {
      const projectLabel = projectLabelForSkill(skill);
      return {
        kind: "skill",
        key: `skill:${skill.scope}:${skill.path}`,
        title: skill.name,
        meta: [pluginEnabledLabel(skill.enabled), projectLabel].filter(Boolean).join(" · "),
        searchText: [skill.name, skill.description, skill.path, projectLabel].join("\n"),
        item: skill,
      };
    });
  }

  if (tab.value === "claude-plugins") {
    return claudePlugins.value.map((plugin) => ({
      kind: "plugin",
      key: `plugin:${plugin.path}`,
      title: plugin.name,
      meta: [pluginEnabledLabel(plugin.enabled), plugin.version ? `v${plugin.version}` : ""]
        .filter(Boolean)
        .join(" · "),
      searchText: [plugin.name, plugin.description, plugin.version, plugin.path].join("\n"),
      item: plugin,
    }));
  }

  if (tab.value === "claude-hooks") {
    return claudeHookSources.value.map((source) => buildHookEntry("claude-hook", source));
  }

  if (tab.value === "claude-mcp") {
    return claudeMcpServers.value.map((server) => ({
      kind: "claude-mcp",
      key: `claude-mcp:${server.name}`,
      title: server.name,
      meta: pluginEnabledLabel(server.enabled),
      searchText: [server.name, server.command, server.args.join(" "), server.envKeys.join(" ")].join(
        "\n",
      ),
      item: server,
    }));
  }

  if (tab.value === "codex-hooks") {
    return codexHookSources.value.map((source) => buildHookEntry("codex-hook", source));
  }

  return codexServers.value.map((server) => ({
    kind: "codex-mcp",
    key: `codex-mcp:${server.name}`,
    title: server.name,
    meta: [
      server.editable ? pluginEnabledLabel(server.enabled) : "只读",
      pluginMcpTransportLabel(server.transport),
    ].join(" · "),
    searchText: [
      server.name,
      server.transport,
      server.command,
      server.args.join(" "),
      server.envKeys.join(" "),
    ].join("\n"),
    item: server,
  }));
});

const filteredEntries = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase();
  if (!needle) return allEntries.value;
  return allEntries.value.filter((entry) =>
    entry.searchText.toLocaleLowerCase().includes(needle),
  );
});

const selectedEntry = computed(
  () => filteredEntries.value.find((entry) => entry.key === selectedKey.value) ?? null,
);

const selectedHookSource = computed(() => (
  isHookEntry(selectedEntry.value) ? selectedEntry.value.item : null
));

const selectedHookSourceData = computed(() => {
  const selected = selectedHookSource.value;
  if (!selected) return null;
  return hookDocumentFor(selected)?.source ?? selected;
});

const selectedHookHandlers = computed(() => {
  const selected = selectedHookSource.value;
  return hookDocumentFor(selected)?.handlers ?? [];
});

const selectedDetailWarnings = computed(() => {
  const source = selectedHookSourceData.value;
  if (!source) return [];
  return mergeUniqueTexts(source.warnings, hookDocumentFor(source)?.warnings ?? []);
});

const selectedDetailLimitations = computed(() => {
  const source = selectedHookSourceData.value;
  if (!source) return [];
  return mergeUniqueTexts(source.limitations, hookDocumentFor(source)?.limitations ?? []);
});

const selectedMeta = computed(() => {
  const entry = selectedEntry.value;
  if (!entry) return "";
  if (entry.kind === "skill") return `${entry.item.scope === "user" ? "全局" : "项目"} · ${entry.meta}`;
  if (entry.kind === "plugin") return `Claude Plugin · ${entry.meta}`;
  if (entry.kind === "claude-mcp") return `${pluginMcpBackendLabel(entry.item.backend)} · ${entry.meta}`;
  if (entry.kind === "codex-mcp") return `${pluginMcpBackendLabel(entry.item.backend)} · ${entry.meta}`;
  const backendLabel = entry.kind === "claude-hook" ? "Claude Hooks" : "Codex Hooks";
  return `${backendLabel} · ${entry.meta}`;
});

const detailRows = computed<DetailRow[]>(() => {
  const entry = selectedEntry.value;
  if (!entry) return [];
  if (entry.kind === "skill") {
    return [
      { label: "描述", value: entry.item.description || "无描述" },
      { label: "路径", value: entry.item.path, code: true },
      { label: "Scope", value: entry.item.scope === "user" ? "全局" : "项目" },
      ...(entry.item.scope === "project"
        ? [{ label: "项目", value: projectLabelForSkill(entry.item) }]
        : []),
      { label: "位置", value: skillLocation(entry.item), code: true },
    ];
  }
  if (entry.kind === "plugin") {
    return [
      { label: "描述", value: entry.item.description || "无描述" },
      { label: "版本", value: entry.item.version || "-" },
      { label: "路径", value: entry.item.path, code: true },
    ];
  }
  if (entry.kind === "claude-mcp") {
    return [
      { label: "命令", value: commandLine(entry.item.command, entry.item.args) || "-", code: true },
      { label: "环境变量", value: entry.item.envKeys.join(", ") || "-" },
      {
        label: "配置",
        value: claudeMcpConfigPath.value || "~/.lilia/config/claude-mcp-servers.json",
        code: true,
      },
    ];
  }
  if (entry.kind === "codex-mcp") {
    return [
      { label: "Transport", value: pluginMcpTransportLabel(entry.item.transport) },
      { label: "命令", value: codexServerSummary(entry.item), code: true },
      { label: "环境变量", value: entry.item.envKeys.join(", ") || "-" },
      { label: "权限", value: entry.item.editable ? "可编辑" : "只读" },
      { label: "配置", value: codexConfigPath.value || "~/.codex/config.toml", code: true },
    ];
  }
  const source = selectedHookSourceData.value ?? entry.item;
  return [
    { label: "Scope", value: hookScopeLabel(source.scope) },
    { label: "格式", value: hookSourceFormatLabel(source.format) },
    { label: "路径", value: source.path, code: true },
    { label: "状态", value: hookSourceStateLabel(source) },
    { label: "权限", value: hookSourceEditLabel(source) },
    { label: "Trust", value: hookTrustStateLabel(source.trustState) },
    { label: "Handlers", value: String(source.handlerCount) },
    ...(source.description ? [{ label: "说明", value: source.description }] : []),
  ];
});

const emptyText = computed(() => {
  if (query.value.trim()) return "没有匹配项";
  if (tab.value === "claude-skills") return "没有 Skill";
  if (tab.value === "claude-plugins") return "没有 Plugin";
  if (tab.value === "claude-hooks" || tab.value === "codex-hooks") return "没有 Hooks 来源";
  return "没有 MCP";
});

function handlerMeta(handler: HookHandlerView) {
  return [
    handler.matcher ? `matcher: ${handler.matcher}` : "",
    handler.type,
    handler.supported ? "" : "未结构化支持",
    handler.executable ? "" : "当前不会执行",
    handler.warnings.length ? `${handler.warnings.length} 警告` : "",
  ].filter(Boolean).join(" · ");
}

watch(
  filteredEntries,
  (entries) => {
    if (entries.some((entry) => entry.key === selectedKey.value)) return;
    selectedKey.value = entries[0]?.key ?? null;
  },
  { immediate: true },
);

watch(tab, () => {
  query.value = "";
});

watch(
  selectedHookSource,
  async (source) => {
    hookDocumentRequestId += 1;
    const requestId = hookDocumentRequestId;
    if (disposed) return;
    selectedHookDocument.value = null;
    hookDocumentError.value = null;
    if (!source) {
      hookDocumentLoading.value = false;
      return;
    }
    hookDocumentLoading.value = true;
    try {
      const document = await readHookSource(source);
      if (disposed || requestId !== hookDocumentRequestId) return;
      selectedHookDocument.value = document;
    } catch (err) {
      if (disposed || requestId !== hookDocumentRequestId) return;
      hookDocumentError.value = String(err);
    } finally {
      if (!disposed && requestId === hookDocumentRequestId) {
        hookDocumentLoading.value = false;
      }
    }
  },
  { immediate: true },
);

function openCreate() {
  if (disposed) return;
  newName.value = "";
  newDesc.value = "";
  createError.value = null;
  void loadSkillCreateDialog();
  showCreate.value = true;
}

async function confirmCreate() {
  if (disposed || creating.value) return;
  createError.value = null;
  creating.value = true;
  try {
    await createSkill(
      "user",
      null,
      newName.value,
      newDesc.value,
    );
    if (disposed) return;
    showCreate.value = false;
    await refresh();
  } catch (err) {
    if (!disposed) createError.value = String(err);
  } finally {
    if (!disposed) creating.value = false;
  }
}

async function toggleSkill(skill: PluginSkill) {
  if (disposed) return;
  try {
    await setSkillEnabled(
      skill.scope,
      skill.scope === "project" ? projectCwd.value : null,
      skill.name,
      !skill.enabled,
    );
    if (disposed) return;
    skill.enabled = !skill.enabled;
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function togglePlugin(plugin: PluginPackage) {
  if (disposed) return;
  try {
    await setPackageEnabled(plugin.backend, plugin.scope, plugin.name, !plugin.enabled);
    if (disposed) return;
    plugin.enabled = !plugin.enabled;
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function toggleMcp(server: PluginMcpServer) {
  if (disposed || !server.editable) return;
  try {
    await setMcpServerEnabled(server.backend, server.name, !server.enabled);
    if (disposed) return;
    server.enabled = !server.enabled;
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function createSelectedHookSource(entry: HookEntry) {
  if (disposed) return;
  try {
    await createHookSource(
      entry.item.backend,
      entry.item.scope,
      entry.item.scope === "project" || entry.item.scope === "local" ? projectCwd.value : null,
    );
    if (disposed) return;
    await refresh();
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function openSelectedHookEditor(entry: HookEntry) {
  if (disposed) return;
  try {
    const document = hookDocumentFor(entry.item) ?? await readHookSource(entry.item);
    if (disposed) return;
    selectedHookDocument.value = document;
    hookDocumentError.value = null;
    void loadHookSourceEditorDialog();
    hookEditor.openHookEditor(document);
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function confirmRemove() {
  if (disposed || removing.value) return;
  removing.value = true;
  try {
    if (pendingRemoveSkill.value) {
      const skill = pendingRemoveSkill.value;
      await deleteSkill(
        skill.scope,
        skill.scope === "project" ? projectCwd.value : null,
        skill.name,
      );
      if (disposed) return;
      pendingRemoveSkill.value = null;
    } else if (pendingRemoveMcp.value) {
      await deleteMcpServer(pendingRemoveMcp.value.backend, pendingRemoveMcp.value.name);
      if (disposed) return;
      pendingRemoveMcp.value = null;
    } else if (pendingRemoveHook.value) {
      await deleteHookSource(pendingRemoveHook.value);
      if (disposed) return;
      pendingRemoveHook.value = null;
    }
    await refresh();
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  } finally {
    if (!disposed) removing.value = false;
  }
}

async function openMcp(server: PluginMcpServer) {
  if (disposed) return;
  try {
    await openMcpConfig(server.backend);
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

async function openHookSourceConfig(source: HookSourceSummary) {
  if (disposed) return;
  try {
    await openHookConfig(source);
  } catch (err) {
    if (!disposed) errorText.value = String(err);
  }
}

function selectEntry(entry: PluginEntry) {
  selectedKey.value = entry.key;
}

function openMcpCreate() {
  if (disposed) return;
  void loadMcpServerEditorDialog();
  if (tab.value === "claude-mcp") mcpEditor.openCreateMcp();
  else if (tab.value === "codex-mcp") codexMcpEditor.openCreateMcp();
}

function canCreateInDetail() {
  if (tab.value === "claude-mcp" || tab.value === "codex-mcp") return true;
  return !!selectedHookSource.value && selectedHookSource.value.editable && !selectedHookSource.value.exists;
}

function detailCreateLabel() {
  return tab.value === "claude-mcp" || tab.value === "codex-mcp" ? "新增 MCP" : "创建来源";
}

function openDetailCreate() {
  if (selectedEntry.value && isHookEntry(selectedEntry.value)) {
    void createSelectedHookSource(selectedEntry.value);
    return;
  }
  openMcpCreate();
}

function toggleSelected(entry: PluginEntry) {
  if (entry.kind === "skill") void toggleSkill(entry.item);
  else if (entry.kind === "plugin") void togglePlugin(entry.item);
  else if (entry.kind === "claude-mcp" || entry.kind === "codex-mcp") void toggleMcp(entry.item);
}

function editSelected(entry: PluginEntry) {
  if (entry.kind === "claude-mcp") {
    void loadMcpServerEditorDialog();
    mcpEditor.openEditMcp(entry.item);
  } else if (entry.kind === "codex-mcp" && entry.item.editable) {
    void loadMcpServerEditorDialog();
    codexMcpEditor.openEditMcp(entry.item);
  }
  else if (isHookEntry(entry)) void openSelectedHookEditor(entry);
}

function removeSelected(entry: PluginEntry) {
  if (disposed) return;
  if (entry.kind === "skill") pendingRemoveSkill.value = entry.item;
  else if (entry.kind === "claude-mcp") pendingRemoveMcp.value = entry.item;
  else if (entry.kind === "codex-mcp" && entry.item.editable) {
    pendingRemoveMcp.value = entry.item;
  } else if (isHookEntry(entry)) {
    pendingRemoveHook.value = entry.item;
  }
}

function openSelectedConfig(entry: PluginEntry) {
  if (entry.kind === "claude-mcp" || entry.kind === "codex-mcp") void openMcp(entry.item);
  else if (isHookEntry(entry)) void openHookSourceConfig(entry.item);
}

onBeforeUnmount(() => {
  disposed = true;
  hookDocumentRequestId += 1;
});

function actionLabel(entry: PluginEntry) {
  return pluginToggleActionLabel(entry.item.enabled);
}

function canToggle(entry: PluginEntry) {
  if (isHookEntry(entry)) return false;
  return entry.kind !== "codex-mcp" || entry.item.editable;
}

function canEdit(entry: PluginEntry) {
  if (isHookEntry(entry)) return entry.item.editable && entry.item.exists;
  return entry.kind === "claude-mcp" || (entry.kind === "codex-mcp" && entry.item.editable);
}

function canRemove(entry: PluginEntry) {
  if (isHookEntry(entry)) return entry.item.editable && entry.item.exists;
  return entry.kind === "skill" || entry.kind === "claude-mcp" || (entry.kind === "codex-mcp" && entry.item.editable);
}

function canOpenConfig(entry: PluginEntry) {
  return entry.kind === "claude-mcp" || entry.kind === "codex-mcp" || isHookEntry(entry);
}
</script>

<template>
  <section class="plugins-page">
    <div class="plugins-browser">
      <div v-if="errorText" class="conn-banner conn-banner--err plugins-browser__banner">
        <AlertTriangle :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">操作失败</div>
          <div class="conn-banner__hint">{{ errorText }}</div>
        </div>
      </div>

      <div v-if="warnings.length" class="conn-banner conn-banner--warn plugins-browser__banner">
        <AlertTriangle :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">解析期警告</div>
          <ul class="plugins-warning-list">
            <li v-for="(w, i) in warnings" :key="i">{{ w }}</li>
          </ul>
        </div>
      </div>

      <div class="plugins-browser__topbar">
        <PluginsTabBar
          v-model="tab"
          :skills-count="userSkills.length + projectSkills.length"
          :plugins-count="claudePlugins.length"
          :claude-hooks-count="claudeHookSources.length"
          :claude-mcp-count="claudeMcpServers.length"
          :codex-hooks-count="codexHookSources.length"
          :codex-mcp-count="codexServers.length"
        />
        <div class="plugins-browser__topbar-actions">
          <button
            v-if="tab === 'claude-skills'"
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="plugins.skill.create"
            @click="openCreate"
          >
            <Plus :size="14" aria-hidden="true" />
            <span>新建 Skill</span>
          </button>
        </div>
      </div>

      <div class="plugins-browser__content">
        <aside class="plugins-browser__sidebar" aria-label="插件和技能列表">
          <div class="plugins-browser__search">
            <label class="plugins-browser__searchbox">
              <Search :size="14" aria-hidden="true" />
              <input
                v-model="query"
                type="search"
                placeholder="搜索当前列表"
                aria-label="搜索插件和技能"
                data-agent-id="plugins.search"
              />
            </label>
          </div>

          <section class="plugins-browser__list ui-list" aria-label="检索结果">
            <div v-if="loading && !filteredEntries.length" class="plugins-browser__notice">
              <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
              <span>读取中</span>
            </div>
            <div v-else-if="!filteredEntries.length" class="plugins-browser__notice">
              {{ emptyText }}
            </div>
            <template v-else>
              <button
                v-for="entry in filteredEntries"
                :key="entry.key"
                type="button"
                class="plugins-browser__row ui-list-item"
                :class="{ 'is-active': selectedKey === entry.key, 'is-disabled': !entry.item.enabled }"
                :data-agent-id="`plugins.entry.${entry.key}`"
                :title="entry.title"
                @click="selectEntry(entry)"
              >
                <span class="plugins-browser__row-title">{{ entry.title }}</span>
                <span class="plugins-browser__row-meta">{{ entry.meta }}</span>
              </button>
            </template>
          </section>
        </aside>

        <section class="plugins-browser__detail" aria-label="插件和技能详情">
          <template v-if="selectedEntry">
            <div class="plugins-browser__detail-head">
              <div class="plugins-browser__detail-heading">
                <div class="plugins-browser__detail-title">{{ selectedEntry.title }}</div>
                <div class="plugins-browser__detail-meta">{{ selectedMeta }}</div>
              </div>
              <div class="plugins-browser__actions">
                <button
                  v-if="canCreateInDetail()"
                  type="button"
                  class="ui-button ui-button--ghost"
                  data-agent-id="plugins.detail.create"
                  @click="openDetailCreate"
                >
                  <Plus :size="14" aria-hidden="true" />
                  <span>{{ detailCreateLabel() }}</span>
                </button>
                <button
                  v-if="canOpenConfig(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost"
                  data-agent-id="plugins.detail.open-config"
                  @click="openSelectedConfig(selectedEntry)"
                >
                  <FolderOpen :size="14" aria-hidden="true" />
                  <span>打开配置</span>
                </button>
                <button
                  v-if="canEdit(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost"
                  data-agent-id="plugins.detail.edit"
                  @click="editSelected(selectedEntry)"
                >
                  <Pencil :size="14" aria-hidden="true" />
                  <span>编辑</span>
                </button>
                <button
                  v-if="canRemove(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost ui-button--danger"
                  data-agent-id="plugins.detail.remove"
                  @click="removeSelected(selectedEntry)"
                >
                  <Trash2 :size="14" aria-hidden="true" />
                  <span>删除</span>
                </button>
                <button
                  v-if="canToggle(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--primary"
                  data-agent-id="plugins.detail.toggle"
                  @click="toggleSelected(selectedEntry)"
                >
                  <Check :size="14" aria-hidden="true" />
                  <span>{{ actionLabel(selectedEntry) }}</span>
                </button>
              </div>
            </div>

            <div class="plugins-browser__detail-list">
              <dl class="plugins-browser__detail-grid">
                <div
                  v-for="row in detailRows"
                  :key="row.label"
                  class="plugins-browser__detail-row"
                >
                  <dt>{{ row.label }}</dt>
                  <dd>
                    <code v-if="row.code">{{ row.value }}</code>
                    <span v-else>{{ row.value }}</span>
                  </dd>
                </div>
              </dl>

              <template v-if="selectedHookSource">
                <div v-if="hookDocumentLoading" class="plugins-browser__notice">
                  <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
                  <span>读取 Hooks 文档…</span>
                </div>
                <div v-else-if="hookDocumentError" class="plugins-hook-section">
                  <div class="plugins-hook-section__title">读取失败</div>
                  <div class="plugins-hook-section__card plugins-hook-section__card--warn">
                    {{ hookDocumentError }}
                  </div>
                </div>
                <div v-if="selectedDetailWarnings.length" class="plugins-hook-section">
                  <div class="plugins-hook-section__title">Warnings</div>
                  <ul class="plugins-hook-section__list">
                    <li v-for="warning in selectedDetailWarnings" :key="warning">{{ warning }}</li>
                  </ul>
                </div>
                <div v-if="selectedDetailLimitations.length" class="plugins-hook-section">
                  <div class="plugins-hook-section__title">限制与说明</div>
                  <ul class="plugins-hook-section__list">
                    <li v-for="limitation in selectedDetailLimitations" :key="limitation">{{ limitation }}</li>
                  </ul>
                </div>
                <div class="plugins-hook-section">
                  <div class="plugins-hook-section__title">Handlers</div>
                  <div v-if="!selectedHookHandlers.length" class="plugins-browser__notice">
                    {{ selectedHookSourceData?.exists ? "当前来源没有 Handler" : "当前来源尚未创建" }}
                  </div>
                  <div v-else class="plugins-hook-handlers">
                    <article
                      v-for="handler in selectedHookHandlers"
                      :key="handler.id"
                      class="plugins-hook-handler"
                    >
                      <div class="plugins-hook-handler__head">
                        <strong>{{ handler.event }}</strong>
                        <span class="plugins-browser__row-meta">{{ handlerMeta(handler) }}</span>
                      </div>
                      <div class="plugins-hook-handler__body">
                        <code v-if="handler.command">{{ handler.command }}</code>
                        <code v-if="handler.commandWindows">{{ handler.commandWindows }}</code>
                        <span v-if="handler.statusMessage">{{ handler.statusMessage }}</span>
                        <span v-if="handler.timeoutSeconds != null">timeout: {{ handler.timeoutSeconds }}s</span>
                      </div>
                      <ul v-if="handler.warnings.length" class="plugins-hook-section__list">
                        <li v-for="warning in handler.warnings" :key="warning">{{ warning }}</li>
                      </ul>
                      <pre v-if="handler.groupAdvancedJson" class="plugins-hook-raw">{{ handler.groupAdvancedJson }}</pre>
                      <pre v-if="handler.advancedJson" class="plugins-hook-raw">{{ handler.advancedJson }}</pre>
                    </article>
                  </div>
                </div>
                <div class="plugins-hook-section">
                  <div class="plugins-hook-section__title">
                    原始文档
                    <span v-if="selectedHookDocument?.rawFormat" class="plugins-browser__row-meta">
                      {{ selectedHookDocument.rawFormat.toUpperCase() }}
                    </span>
                  </div>
                  <pre v-if="selectedHookDocument?.rawDocument" class="plugins-hook-raw">{{ selectedHookDocument.rawDocument }}</pre>
                  <div v-else class="plugins-browser__notice">当前来源没有原始文档内容</div>
                </div>
              </template>
            </div>
          </template>
          <template v-else>
            <div class="plugins-browser__detail-head">
              <div class="plugins-browser__detail-heading">
                <div class="plugins-browser__detail-title">未选择</div>
              </div>
              <div class="plugins-browser__actions">
                <button
                  v-if="canCreateInDetail()"
                  type="button"
                  class="ui-button ui-button--ghost"
                  data-agent-id="plugins.detail.create"
                  @click="openDetailCreate"
                >
                  <Plus :size="14" aria-hidden="true" />
                  <span>{{ detailCreateLabel() }}</span>
                </button>
              </div>
            </div>
            <div class="plugins-browser__empty-detail">选择一项</div>
          </template>
        </section>
      </div>
    </div>

    <SkillCreateDialog
      v-if="showCreate"
      v-model:open="showCreate"
      v-model:name="newName"
      v-model:description="newDesc"
      :scope-hint="USER_SKILLS_ROOT"
      :creating="creating"
      :error="createError"
      @confirm="confirmCreate"
    />

    <HookSourceEditorDialog
      v-if="hookEditor.showHookEditor.value"
      v-model:open="hookEditor.showHookEditor.value"
      :title="hookEditor.hookEditorTitle.value"
      :source-name="hookEditor.editingSource.value?.name ?? ''"
      :source-path="hookEditor.editingSource.value?.path ?? null"
      :handler-rows="hookEditor.hookHandlerRows.value"
      :saving="hookEditor.hookSaving.value"
      :error="hookEditor.hookError.value"
      @add-handler="hookEditor.addHookHandler"
      @remove-handler="hookEditor.removeHookHandler"
      @confirm="hookEditor.saveHookSource"
    />

    <McpServerEditorDialog
      v-if="mcpEditor.showMcpEditor.value"
      v-model:open="mcpEditor.showMcpEditor.value"
      v-model:name="mcpEditor.mcpName.value"
      v-model:command="mcpEditor.mcpCommand.value"
      v-model:args-text="mcpEditor.mcpArgsText.value"
      :env-rows="mcpEditor.mcpEnvRows.value"
      :editing-mcp="mcpEditor.editingMcp.value"
      :title="mcpEditor.mcpEditorTitle.value"
      server-label="Claude MCP"
      :saving="mcpEditor.mcpSaving.value"
      :error="mcpEditor.mcpError.value"
      :config-path="claudeMcpConfigPath || '~/.lilia/config/claude-mcp-servers.json'"
      @add-env-row="mcpEditor.addMcpEnvRow"
      @remove-env-row="mcpEditor.removeMcpEnvRow"
      @confirm="mcpEditor.saveMcpServer"
    />

    <McpServerEditorDialog
      v-if="codexMcpEditor.showMcpEditor.value"
      v-model:open="codexMcpEditor.showMcpEditor.value"
      v-model:name="codexMcpEditor.mcpName.value"
      v-model:command="codexMcpEditor.mcpCommand.value"
      v-model:args-text="codexMcpEditor.mcpArgsText.value"
      :env-rows="codexMcpEditor.mcpEnvRows.value"
      :editing-mcp="codexMcpEditor.editingMcp.value"
      :title="codexMcpEditor.mcpEditorTitle.value"
      server-label="Codex MCP"
      :saving="codexMcpEditor.mcpSaving.value"
      :error="codexMcpEditor.mcpError.value"
      :config-path="codexConfigPath || '~/.codex/config.toml'"
      @add-env-row="codexMcpEditor.addMcpEnvRow"
      @remove-env-row="codexMcpEditor.removeMcpEnvRow"
      @confirm="codexMcpEditor.saveMcpServer"
    />

    <ConfirmDialog
      :open="pendingRemoveSkill !== null || pendingRemoveMcp !== null || pendingRemoveHook !== null"
      :title="pendingRemoveSkill
        ? `删除 skill「${pendingRemoveSkill?.name ?? ''}」？`
        : pendingRemoveMcp
          ? `删除 ${pluginMcpBackendLabel(pendingRemoveMcp.backend)}「${pendingRemoveMcp?.name ?? ''}」？`
          : `删除 Hooks 来源「${pendingRemoveHook?.name ?? ''}」？`"
      :message="pendingRemoveSkill
        ? '该 skill 目录会被整体删除，不可恢复。'
        : pendingRemoveMcp
          ? pendingRemoveMcp.backend === 'claude'
            ? '该 MCP server 会从 Lilia 配置中删除，不可恢复。'
            : '该 stdio MCP server 会从 Codex config.toml 中删除，不可恢复。'
          : '该 hooks 来源会从对应配置文件中移除，不可恢复。'"
      confirm-text="删除"
      busy-text="删除中…"
      :busy="removing"
      danger
      @cancel="pendingRemoveSkill = null; pendingRemoveMcp = null; pendingRemoveHook = null"
      @confirm="confirmRemove"
    />
  </section>
</template>

