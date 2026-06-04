<script setup lang="ts">
import { ref } from "vue";
import { AlertTriangle, RefreshCw } from "lucide-vue-next";
import ConfirmDialog from "../components/ConfirmDialog.vue";
import {
  createClaudeSkill,
  deleteClaudeMcpServer,
  deleteClaudeSkill,
  openClaudeMcpConfig,
  openCodexConfig,
  setClaudeMcpServerEnabled,
  setClaudePluginEnabled,
  setClaudeSkillEnabled,
  type ClaudeMcpServer,
  type ClaudePlugin,
  type ClaudeSkill,
} from "../services/plugins";
import { usePluginsOverview } from "./plugins/usePluginsOverview";
import { useClaudeMcpEditor } from "./plugins/useClaudeMcpEditor";
import PluginsTabBar from "./plugins/PluginsTabBar.vue";
import ClaudeSkillsTab from "./plugins/ClaudeSkillsTab.vue";
import ClaudePluginsTab from "./plugins/ClaudePluginsTab.vue";
import ClaudeMcpTab from "./plugins/ClaudeMcpTab.vue";
import CodexMcpTab from "./plugins/CodexMcpTab.vue";
import SkillCreateDialog from "./plugins/SkillCreateDialog.vue";
import ClaudeMcpEditorDialog from "./plugins/ClaudeMcpEditorDialog.vue";

const {
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
} = usePluginsOverview();

const showCreate = ref(false);
const newName = ref("");
const newDesc = ref("");
const creating = ref(false);
const createError = ref<string | null>(null);
const pendingRemoveSkill = ref<ClaudeSkill | null>(null);
const pendingRemoveMcp = ref<ClaudeMcpServer | null>(null);
const removing = ref(false);

const mcpEditor = useClaudeMcpEditor({ refresh });

function openCreate() {
  newName.value = "";
  newDesc.value = "";
  createError.value = null;
  showCreate.value = true;
}

async function confirmCreate() {
  if (creating.value) return;
  createError.value = null;
  if (scope.value === "project" && !projectCwd.value) {
    createError.value = "项目 scope 需要先选择一个项目";
    return;
  }
  creating.value = true;
  try {
    await createClaudeSkill(
      scope.value,
      scope.value === "project" ? projectCwd.value : null,
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

async function toggleSkill(skill: ClaudeSkill) {
  try {
    await setClaudeSkillEnabled(
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

async function togglePlugin(plugin: ClaudePlugin) {
  try {
    await setClaudePluginEnabled(plugin.scope, plugin.name, !plugin.enabled);
    plugin.enabled = !plugin.enabled;
  } catch (err) {
    errorText.value = String(err);
  }
}

async function toggleMcp(server: ClaudeMcpServer) {
  try {
    await setClaudeMcpServerEnabled(server.name, !server.enabled);
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
      await deleteClaudeSkill(
        skill.scope,
        skill.scope === "project" ? projectCwd.value : null,
        skill.name,
      );
      pendingRemoveSkill.value = null;
    } else if (pendingRemoveMcp.value) {
      await deleteClaudeMcpServer(pendingRemoveMcp.value.name);
      pendingRemoveMcp.value = null;
    }
    await refresh();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    removing.value = false;
  }
}

async function openClaudeMcp() {
  try {
    await openClaudeMcpConfig();
  } catch (err) {
    errorText.value = String(err);
  }
}

async function openCodex() {
  try {
    await openCodexConfig();
  } catch (err) {
    errorText.value = String(err);
  }
}
</script>

<template>
  <section class="plugins-page">
    <div class="page-header">
      <div>
        <h1>插件 / 技能</h1>
        <p>统一管理 Claude 的 skills 与 plugins，以及 Codex 的 MCP servers。</p>
      </div>
      <div class="page-header__actions">
        <button type="button" class="ghost" :disabled="loading" @click="refresh">
          <RefreshCw :size="14" :class="loading ? 'is-spinning' : ''" aria-hidden="true" />
          {{ loading ? "刷新中…" : "刷新" }}
        </button>
      </div>
    </div>

    <PluginsTabBar
      v-model="tab"
      :skills-count="userSkills.length + projectSkills.length"
      :plugins-count="claudePlugins.length"
      :claude-mcp-count="claudeMcpServers.length"
      :codex-mcp-count="codexServers.length"
    />

    <div v-if="errorText" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">操作失败</div>
        <div class="conn-banner__hint">{{ errorText }}</div>
      </div>
    </div>

    <div v-if="warnings.length" class="conn-banner conn-banner--warn">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">解析期警告</div>
        <ul class="plugins-warning-list">
          <li v-for="(w, i) in warnings" :key="i">{{ w }}</li>
        </ul>
      </div>
    </div>

    <ClaudeSkillsTab
      v-if="tab === 'claude-skills'"
      v-model:scope="scope"
      :project-cwd="projectCwd"
      :project-options="projectOptions"
      :current-skills="currentSkills"
      :current-scope-hint="currentScopeHint"
      @project-change="onProjectChange"
      @create="openCreate"
      @toggle="toggleSkill"
      @remove="pendingRemoveSkill = $event"
    />

    <ClaudePluginsTab
      v-else-if="tab === 'claude-plugins'"
      :plugins="claudePlugins"
      @toggle="togglePlugin"
    />

    <ClaudeMcpTab
      v-else-if="tab === 'claude-mcp'"
      :servers="claudeMcpServers"
      :config-path="claudeMcpConfigPath"
      @open-config="openClaudeMcp"
      @create="mcpEditor.openCreateMcp"
      @edit="mcpEditor.openEditMcp"
      @toggle="toggleMcp"
      @remove="pendingRemoveMcp = $event"
    />

    <CodexMcpTab
      v-else
      :servers="codexServers"
      :config-path="codexConfigPath"
      @open-config="openCodex"
    />

    <SkillCreateDialog
      v-model:open="showCreate"
      v-model:name="newName"
      v-model:description="newDesc"
      :scope-hint="currentScopeHint"
      :creating="creating"
      :error="createError"
      @confirm="confirmCreate"
    />

    <ClaudeMcpEditorDialog
      v-model:open="mcpEditor.showMcpEditor.value"
      v-model:name="mcpEditor.mcpName.value"
      v-model:command="mcpEditor.mcpCommand.value"
      v-model:args-text="mcpEditor.mcpArgsText.value"
      :env-rows="mcpEditor.mcpEnvRows.value"
      :editing-mcp="mcpEditor.editingMcp.value"
      :title="mcpEditor.mcpEditorTitle.value"
      :saving="mcpEditor.mcpSaving.value"
      :error="mcpEditor.mcpError.value"
      :config-path="claudeMcpConfigPath"
      @add-env-row="mcpEditor.addMcpEnvRow"
      @remove-env-row="mcpEditor.removeMcpEnvRow"
      @confirm="mcpEditor.saveMcpServer"
    />

    <ConfirmDialog
      :open="pendingRemoveSkill !== null || pendingRemoveMcp !== null"
      :title="pendingRemoveSkill
        ? `删除 skill「${pendingRemoveSkill?.name ?? ''}」？`
        : `删除 Claude MCP「${pendingRemoveMcp?.name ?? ''}」？`"
      :message="pendingRemoveSkill
        ? '该 skill 目录会被整体删除，不可恢复。'
        : '该 MCP server 会从 Lilia 配置中删除，不可恢复。'"
      confirm-text="删除"
      busy-text="删除中…"
      :busy="removing"
      danger
      @cancel="pendingRemoveSkill = null; pendingRemoveMcp = null"
      @confirm="confirmRemove"
    />
  </section>
</template>
