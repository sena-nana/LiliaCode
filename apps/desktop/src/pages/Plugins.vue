<script setup lang="ts">
/**
 * 插件 / 技能管理面板。
 *
 * 两块 backend 的扩展机制不对称，UI 用 tab 切：
 * - Claude Skills（user / project scope 二级切换）
 * - Claude Plugins（marketplace beta，仅只读列出）
 * - Codex MCP servers（来自 ~/.codex/config.toml，只读 + 打开文件按钮）
 *
 * 没接真的 marketplace；「安装插件」按钮先给个 placeholder 弹层占位。
 * Codex 那边不做运行时启停，把它的复杂性留给用户和文本编辑器。
 */
import { computed, onMounted, ref } from "vue";
import {
  Puzzle, Sparkles, Server, Plus, RefreshCw, Trash2, FolderOpen,
  AlertTriangle, Check, Layers, Folder,
} from "lucide-vue-next";
import {
  pluginsOverview,
  createClaudeSkill,
  deleteClaudeSkill,
  setClaudeSkillEnabled,
  openCodexConfig,
  type ClaudePlugin,
  type ClaudeSkill,
  type CodexMcpServer,
  type PluginScope,
} from "../services/plugins";
import { listProjects } from "../data/projectsStub";
import Dropdown from "../components/Dropdown.vue";
import ConfirmDialog from "../components/ConfirmDialog.vue";

type Tab = "claude-skills" | "claude-plugins" | "codex-mcp";

const tab = ref<Tab>("claude-skills");
const scope = ref<PluginScope>("user");

const projects = computed(() => listProjects());
const projectCwd = ref<string | null>(projects.value[0]?.cwd ?? null);
const projectOptions = computed(() =>
  projects.value.map((p) => ({ value: p.cwd, label: p.name, hint: p.cwd })),
);

function onProjectChange(cwd: string) {
  projectCwd.value = cwd;
  refresh();
}

const userSkills = ref<ClaudeSkill[]>([]);
const projectSkills = ref<ClaudeSkill[]>([]);
const claudePlugins = ref<ClaudePlugin[]>([]);
const codexServers = ref<CodexMcpServer[]>([]);
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
    codexServers.value = data.codexMcpServers;
    warnings.value = data.warnings;
  } catch (err) {
    errorText.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(() => refresh());

// ---- Skill: 新建对话框 ----
const showCreate = ref(false);
const newName = ref("");
const newDesc = ref("");
const creating = ref(false);
const createError = ref<string | null>(null);

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

// ---- Skill: 启停 / 删除 ----
async function toggleSkill(skill: ClaudeSkill) {
  try {
    await setClaudeSkillEnabled(
      skill.scope as PluginScope,
      skill.scope === "project" ? projectCwd.value : null,
      skill.name,
      !skill.enabled,
    );
    skill.enabled = !skill.enabled;
  } catch (err) {
    errorText.value = String(err);
  }
}

async function removeSkill(skill: ClaudeSkill) {
  pendingRemove.value = skill;
}

const pendingRemove = ref<ClaudeSkill | null>(null);
const removing = ref(false);

async function confirmRemove() {
  const skill = pendingRemove.value;
  if (!skill || removing.value) return;
  removing.value = true;
  try {
    await deleteClaudeSkill(
      skill.scope as PluginScope,
      skill.scope === "project" ? projectCwd.value : null,
      skill.name,
    );
    pendingRemove.value = null;
    await refresh();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    removing.value = false;
  }
}

// ---- Codex: 打开 config.toml ----
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

    <div class="plugins-tabs" role="tablist">
      <button
        type="button" role="tab"
        :aria-selected="tab === 'claude-skills'"
        :class="{ 'is-active': tab === 'claude-skills' }"
        @click="tab = 'claude-skills'"
      >
        <Sparkles :size="14" aria-hidden="true" /> Claude Skills
        <span class="plugins-tabs__count">{{ userSkills.length + projectSkills.length }}</span>
      </button>
      <button
        type="button" role="tab"
        :aria-selected="tab === 'claude-plugins'"
        :class="{ 'is-active': tab === 'claude-plugins' }"
        @click="tab = 'claude-plugins'"
      >
        <Puzzle :size="14" aria-hidden="true" /> Claude Plugins
        <span class="plugins-tabs__count">{{ claudePlugins.length }}</span>
      </button>
      <button
        type="button" role="tab"
        :aria-selected="tab === 'codex-mcp'"
        :class="{ 'is-active': tab === 'codex-mcp' }"
        @click="tab = 'codex-mcp'"
      >
        <Server :size="14" aria-hidden="true" /> Codex MCP
        <span class="plugins-tabs__count">{{ codexServers.length }}</span>
      </button>
    </div>

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

    <!-- ===== Claude Skills ===== -->
    <div v-if="tab === 'claude-skills'" class="card card--allow-overflow">
      <div class="plugins-toolbar">
        <div class="segmented" role="radiogroup" aria-label="Skill scope">
          <button
            type="button" role="radio"
            :aria-checked="scope === 'user'"
            :class="{ 'is-active': scope === 'user' }"
            @click="scope = 'user'"
          >
            <Layers :size="12" aria-hidden="true" /> 全局
          </button>
          <button
            type="button" role="radio"
            :aria-checked="scope === 'project'"
            :class="{ 'is-active': scope === 'project' }"
            @click="scope = 'project'"
          >
            <Layers :size="12" aria-hidden="true" /> 项目
          </button>
        </div>

        <Dropdown
          v-if="scope === 'project' && projectCwd"
          :model-value="projectCwd"
          :options="projectOptions"
          :icon="Folder"
          placement="bottom"
          @update:model-value="onProjectChange"
        />
        <span v-else-if="scope === 'project'" class="muted">尚未配置项目</span>

        <span class="plugins-toolbar__hint">{{ currentScopeHint }}</span>

        <button type="button" class="primary" @click="openCreate">
          <Plus :size="14" aria-hidden="true" /> 新建 Skill
        </button>
      </div>

      <ul v-if="currentSkills.length" class="plugins-list">
        <li
          v-for="s in currentSkills"
          :key="s.path"
          class="plugins-list__item"
          :class="{ 'is-disabled': !s.enabled }"
        >
          <div class="plugins-list__head">
            <span class="plugins-list__name">{{ s.name }}</span>
            <span v-if="s.enabled" class="plugins-list__badge plugins-list__badge--ok">
              <Check :size="11" aria-hidden="true" /> 已启用
            </span>
            <span v-else class="plugins-list__badge plugins-list__badge--mute">已停用</span>
          </div>
          <p class="plugins-list__desc">{{ s.description || "（无描述）" }}</p>
          <div class="plugins-list__meta">
            <code>{{ s.path }}</code>
          </div>
          <div class="plugins-list__actions">
            <button type="button" class="ghost" @click="toggleSkill(s)">
              {{ s.enabled ? "停用" : "启用" }}
            </button>
            <button type="button" class="ghost danger" @click="removeSkill(s)">
              <Trash2 :size="12" aria-hidden="true" /> 删除
            </button>
          </div>
        </li>
      </ul>
      <p v-else class="plugins-empty">
        {{ scope === 'user' ? '全局' : '项目' }} skill 目录里还没有任何 SKILL.md。
      </p>
    </div>

    <!-- ===== Claude Plugins ===== -->
    <div v-else-if="tab === 'claude-plugins'" class="card">
      <p class="plugins-section-hint">
        来自 <code>~/.claude/plugins/</code> 的 marketplace 插件（beta）。一期仅展示，
        后续接入安装 / 升级 接口时这里会出现操作按钮。
      </p>
      <ul v-if="claudePlugins.length" class="plugins-list">
        <li v-for="p in claudePlugins" :key="p.path" class="plugins-list__item">
          <div class="plugins-list__head">
            <span class="plugins-list__name">{{ p.name }}</span>
            <span class="plugins-list__badge plugins-list__badge--mute">v{{ p.version || "—" }}</span>
          </div>
          <p class="plugins-list__desc">{{ p.description || "（无描述）" }}</p>
          <div class="plugins-list__meta"><code>{{ p.path }}</code></div>
        </li>
      </ul>
      <p v-else class="plugins-empty">没有发现已安装的 plugin。</p>
    </div>

    <!-- ===== Codex MCP ===== -->
    <div v-else class="card">
      <div class="plugins-toolbar">
        <span class="plugins-toolbar__hint">来自 <code>~/.codex/config.toml</code> 的 mcp_servers 节</span>
        <button type="button" class="ghost" @click="openCodex">
          <FolderOpen :size="12" aria-hidden="true" /> 打开 config.toml
        </button>
      </div>
      <ul v-if="codexServers.length" class="plugins-list">
        <li v-for="s in codexServers" :key="s.name" class="plugins-list__item">
          <div class="plugins-list__head">
            <span class="plugins-list__name">{{ s.name }}</span>
            <span class="plugins-list__badge plugins-list__badge--ok">
              <Check :size="11" aria-hidden="true" /> 已注册
            </span>
          </div>
          <div class="plugins-list__meta">
            <code>{{ s.command }} {{ s.args.join(' ') }}</code>
          </div>
        </li>
      </ul>
      <p v-else class="plugins-empty">
        config.toml 里还没有 mcp_servers 节，点击「打开 config.toml」开始添加。
      </p>
    </div>

    <!-- ===== Create Skill 弹层 ===== -->
    <Teleport to="body">
      <Transition name="search-palette">
        <div
          v-if="showCreate"
          class="search-palette"
          role="dialog" aria-modal="true" aria-label="新建 Skill"
          @click.self="showCreate = false"
        >
          <div class="search-palette__card dialog__card">
            <div class="dialog__header">
              <Sparkles :size="14" aria-hidden="true" />
              <span>新建 Claude Skill</span>
            </div>
            <div class="dialog__body">
              <label>
                <span>名称</span>
                <input
                  v-model="newName" type="text"
                  class="text-input"
                  placeholder="kebab-case，仅 a-z 0-9 - _"
                />
              </label>
              <label>
                <span>描述</span>
                <textarea
                  v-model="newDesc"
                  class="text-input"
                  rows="3"
                  placeholder="一行描述，告诉 Claude 什么时候应该用这个 skill"
                />
              </label>
              <p v-if="createError" class="plugins-create__error">{{ createError }}</p>
              <p class="plugins-create__hint">
                创建后会生成 <code>{{ currentScopeHint }}{{ newName || '<name>' }}/SKILL.md</code>。
              </p>
            </div>
            <div class="dialog__actions">
              <button type="button" class="ghost" :disabled="creating" @click="showCreate = false">
                取消
              </button>
              <button type="button" class="primary" :disabled="creating" @click="confirmCreate">
                {{ creating ? "创建中…" : "创建" }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- ===== 删除 Skill 二次确认 ===== -->
    <ConfirmDialog
      :open="pendingRemove !== null"
      :title="`删除 skill「${pendingRemove?.name ?? ''}」？`"
      message="该 skill 目录会被整体删除，不可恢复。"
      confirm-text="删除"
      busy-text="删除中…"
      :busy="removing"
      danger
      @cancel="pendingRemove = null"
      @confirm="confirmRemove"
    />
  </section>
</template>
