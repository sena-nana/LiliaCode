<script setup lang="ts">
import "../styles/pages/plugins.css";
import { computed, ref, watch } from "vue";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  Loader2,
  Pencil,
  Plus,
  Search,
  Trash2,
} from "lucide-vue-next";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import {
  createSkill,
  deleteMcpServer,
  deleteSkill,
  openMcpConfig,
  setMcpServerEnabled,
  setPackageEnabled,
  setSkillEnabled,
  type PluginMcpServer,
  type PluginPackage,
  type PluginSkill,
} from "../services/plugins";
import { usePluginsOverview } from "./plugins/usePluginsOverview";
import { useMcpServerEditor } from "./plugins/useMcpServerEditor";
import PluginsTabBar from "./plugins/PluginsTabBar.vue";
import SkillCreateDialog from "./plugins/SkillCreateDialog.vue";
import McpServerEditorDialog from "./plugins/McpServerEditorDialog.vue";

const {
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
} = usePluginsOverview();

const showCreate = ref(false);
const newName = ref("");
const newDesc = ref("");
const creating = ref(false);
const createError = ref<string | null>(null);
const pendingRemoveSkill = ref<PluginSkill | null>(null);
const pendingRemoveMcp = ref<PluginMcpServer | null>(null);
const removing = ref(false);
const query = ref("");
const selectedKey = ref<string | null>(null);

const mcpEditor = useMcpServerEditor<PluginMcpServer>({
  backend: "claude",
  label: "Claude MCP",
  refresh,
});
const codexMcpEditor = useMcpServerEditor<PluginMcpServer>({
  backend: "codex",
  label: "Codex MCP",
  refresh,
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
    };

interface DetailRow {
  label: string;
  value: string;
  code?: boolean;
}

function enabledLabel(enabled: boolean) {
  return enabled ? "已启用" : "已停用";
}

function codexTransportLabel(server: PluginMcpServer) {
  if (server.transport === "stdio") return "stdio";
  if (server.transport === "http") return "HTTP";
  if (server.transport === "oauth") return "OAuth";
  return "未知";
}

function commandLine(command: string, args: string[]) {
  return [command, ...args].filter(Boolean).join(" ").trim();
}

function codexServerSummary(server: PluginMcpServer) {
  return commandLine(server.command, server.args) || `${codexTransportLabel(server)} MCP server`;
}

function mcpBackendLabel(server: PluginMcpServer) {
  return server.backend === "claude" ? "Claude MCP" : "Codex MCP";
}

function normalizePath(value: string | null | undefined) {
  return (value ?? "").replace(/\\/g, "/").toLocaleLowerCase();
}

function projectFolderFromSkillPath(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const marker = "/.claude/skills/";
  const markerIndex = normalized.toLocaleLowerCase().indexOf(marker);
  if (markerIndex <= 0) return "";
  const projectPath = normalized.slice(0, markerIndex).replace(/\/+$/, "");
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

const allEntries = computed<PluginEntry[]>(() => {
  if (tab.value === "claude-skills") {
    return [...userSkills.value, ...projectSkills.value].map((skill) => {
      const projectLabel = projectLabelForSkill(skill);
      return {
        kind: "skill",
        key: `skill:${skill.scope}:${skill.path}`,
        title: skill.name,
        meta: [enabledLabel(skill.enabled), projectLabel].filter(Boolean).join(" · "),
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
      meta: [enabledLabel(plugin.enabled), plugin.version ? `v${plugin.version}` : ""]
        .filter(Boolean)
        .join(" · "),
      searchText: [plugin.name, plugin.description, plugin.version, plugin.path].join("\n"),
      item: plugin,
    }));
  }

  if (tab.value === "claude-mcp") {
    return claudeMcpServers.value.map((server) => ({
      kind: "claude-mcp",
      key: `claude-mcp:${server.name}`,
      title: server.name,
      meta: enabledLabel(server.enabled),
      searchText: [server.name, server.command, server.args.join(" "), server.envKeys.join(" ")].join(
        "\n",
      ),
      item: server,
    }));
  }

  return codexServers.value.map((server) => ({
    kind: "codex-mcp",
    key: `codex-mcp:${server.name}`,
    title: server.name,
    meta: [
      server.editable ? enabledLabel(server.enabled) : "只读",
      codexTransportLabel(server),
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

const selectedMeta = computed(() => {
  const entry = selectedEntry.value;
  if (!entry) return "";
  if (entry.kind === "skill") return `${entry.item.scope === "user" ? "全局" : "项目"} · ${entry.meta}`;
  if (entry.kind === "plugin") return `Claude Plugin · ${entry.meta}`;
  if (entry.kind === "claude-mcp") return `Claude MCP · ${entry.meta}`;
  return `Codex MCP · ${entry.meta}`;
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
  return [
    { label: "Transport", value: codexTransportLabel(entry.item) },
    { label: "命令", value: codexServerSummary(entry.item), code: true },
    { label: "环境变量", value: entry.item.envKeys.join(", ") || "-" },
    { label: "权限", value: entry.item.editable ? "可编辑" : "只读" },
    { label: "配置", value: codexConfigPath.value || "~/.codex/config.toml", code: true },
  ];
});

const emptyText = computed(() => {
  if (query.value.trim()) return "没有匹配项";
  if (tab.value === "claude-skills") return "没有 Skill";
  if (tab.value === "claude-plugins") return "没有 Plugin";
  return "没有 MCP";
});

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

function openCreate() {
  newName.value = "";
  newDesc.value = "";
  createError.value = null;
  showCreate.value = true;
}

async function confirmCreate() {
  if (creating.value) return;
  createError.value = null;
  creating.value = true;
  try {
    await createSkill(
      "user",
      null,
      newName.value,
      newDesc.value,
    );
    showCreate.value = false;
    await refresh();
  } catch (err) {
    createError.value = String(err);
  } finally {
    creating.value = false;
  }
}

async function toggleSkill(skill: PluginSkill) {
  try {
    await setSkillEnabled(
      skill.scope,
      skill.scope === "project" ? projectCwd.value : null,
      skill.name,
      !skill.enabled,
    );
    skill.enabled = !skill.enabled;
  } catch (err) {
    errorText.value = String(err);
  }
}

async function togglePlugin(plugin: PluginPackage) {
  try {
    await setPackageEnabled(plugin.backend, plugin.scope, plugin.name, !plugin.enabled);
    plugin.enabled = !plugin.enabled;
  } catch (err) {
    errorText.value = String(err);
  }
}

async function toggleMcp(server: PluginMcpServer) {
  if (!server.editable) return;
  try {
    await setMcpServerEnabled(server.backend, server.name, !server.enabled);
    server.enabled = !server.enabled;
  } catch (err) {
    errorText.value = String(err);
  }
}

async function confirmRemove() {
  if (removing.value) return;
  removing.value = true;
  try {
    if (pendingRemoveSkill.value) {
      const skill = pendingRemoveSkill.value;
      await deleteSkill(
        skill.scope,
        skill.scope === "project" ? projectCwd.value : null,
        skill.name,
      );
      pendingRemoveSkill.value = null;
    } else if (pendingRemoveMcp.value) {
      await deleteMcpServer(pendingRemoveMcp.value.backend, pendingRemoveMcp.value.name);
      pendingRemoveMcp.value = null;
    }
    await refresh();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    removing.value = false;
  }
}

async function openMcp(server: PluginMcpServer) {
  try {
    await openMcpConfig(server.backend);
  } catch (err) {
    errorText.value = String(err);
  }
}

function selectEntry(entry: PluginEntry) {
  selectedKey.value = entry.key;
}

function openMcpCreate() {
  if (tab.value === "claude-mcp") mcpEditor.openCreateMcp();
  else if (tab.value === "codex-mcp") codexMcpEditor.openCreateMcp();
}

function canCreateInDetail() {
  return tab.value === "claude-mcp" || tab.value === "codex-mcp";
}

function toggleSelected(entry: PluginEntry) {
  if (entry.kind === "skill") void toggleSkill(entry.item);
  else if (entry.kind === "plugin") void togglePlugin(entry.item);
  else void toggleMcp(entry.item);
}

function editSelected(entry: PluginEntry) {
  if (entry.kind === "claude-mcp") mcpEditor.openEditMcp(entry.item);
  else if (entry.kind === "codex-mcp" && entry.item.editable) codexMcpEditor.openEditMcp(entry.item);
}

function removeSelected(entry: PluginEntry) {
  if (entry.kind === "skill") pendingRemoveSkill.value = entry.item;
  else if (entry.kind === "claude-mcp") pendingRemoveMcp.value = entry.item;
  else if (entry.kind === "codex-mcp" && entry.item.editable) {
    pendingRemoveMcp.value = entry.item;
  }
}

function openSelectedConfig(entry: PluginEntry) {
  if (entry.kind === "claude-mcp" || entry.kind === "codex-mcp") void openMcp(entry.item);
}

function actionLabel(entry: PluginEntry) {
  return entry.item.enabled ? "停用" : "启用";
}

function canToggle(entry: PluginEntry) {
  return entry.kind !== "codex-mcp" || entry.item.editable;
}

function canEdit(entry: PluginEntry) {
  return entry.kind === "claude-mcp" || (entry.kind === "codex-mcp" && entry.item.editable);
}

function canRemove(entry: PluginEntry) {
  return entry.kind === "skill" || entry.kind === "claude-mcp" || (entry.kind === "codex-mcp" && entry.item.editable);
}

function canOpenConfig(entry: PluginEntry) {
  return entry.kind === "claude-mcp" || entry.kind === "codex-mcp";
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
          :claude-mcp-count="claudeMcpServers.length"
          :codex-mcp-count="codexServers.length"
        />
        <div class="plugins-browser__topbar-actions">
          <button
            v-if="tab === 'claude-skills'"
            type="button"
            class="ui-button ui-button--ghost"
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
                  @click="openMcpCreate"
                >
                  <Plus :size="14" aria-hidden="true" />
                  <span>新增 MCP</span>
                </button>
                <button
                  v-if="canOpenConfig(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost"
                  @click="openSelectedConfig(selectedEntry)"
                >
                  <FolderOpen :size="14" aria-hidden="true" />
                  <span>打开配置</span>
                </button>
                <button
                  v-if="canEdit(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost"
                  @click="editSelected(selectedEntry)"
                >
                  <Pencil :size="14" aria-hidden="true" />
                  <span>编辑</span>
                </button>
                <button
                  v-if="canRemove(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--ghost ui-button--danger"
                  @click="removeSelected(selectedEntry)"
                >
                  <Trash2 :size="14" aria-hidden="true" />
                  <span>删除</span>
                </button>
                <button
                  v-if="canToggle(selectedEntry)"
                  type="button"
                  class="ui-button ui-button--primary"
                  @click="toggleSelected(selectedEntry)"
                >
                  <Check :size="14" aria-hidden="true" />
                  <span>{{ actionLabel(selectedEntry) }}</span>
                </button>
              </div>
            </div>

            <dl class="plugins-browser__detail-list">
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
                  @click="openMcpCreate"
                >
                  <Plus :size="14" aria-hidden="true" />
                  <span>新增 MCP</span>
                </button>
              </div>
            </div>
            <div class="plugins-browser__empty-detail">选择一项</div>
          </template>
        </section>
      </div>
    </div>

    <SkillCreateDialog
      v-model:open="showCreate"
      v-model:name="newName"
      v-model:description="newDesc"
      :scope-hint="USER_SKILLS_ROOT"
      :creating="creating"
      :error="createError"
      @confirm="confirmCreate"
    />

    <McpServerEditorDialog
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
      :open="pendingRemoveSkill !== null || pendingRemoveMcp !== null"
      :title="pendingRemoveSkill
        ? `删除 skill「${pendingRemoveSkill?.name ?? ''}」？`
        : `删除 ${pendingRemoveMcp ? mcpBackendLabel(pendingRemoveMcp) : 'MCP'}「${pendingRemoveMcp?.name ?? ''}」？`"
      :message="pendingRemoveSkill
        ? '该 skill 目录会被整体删除，不可恢复。'
        : pendingRemoveMcp?.backend === 'claude'
          ? '该 MCP server 会从 Lilia 配置中删除，不可恢复。'
          : '该 stdio MCP server 会从 Codex config.toml 中删除，不可恢复。'"
      confirm-text="删除"
      busy-text="删除中…"
      :busy="removing"
      danger
      @cancel="pendingRemoveSkill = null; pendingRemoveMcp = null"
      @confirm="confirmRemove"
    />
  </section>
</template>
