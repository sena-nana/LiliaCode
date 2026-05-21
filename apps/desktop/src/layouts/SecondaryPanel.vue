<script setup lang="ts">
import { computed, reactive } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Settings,
  MessageSquarePlus,
  Search,
  Puzzle,
  Zap,
  Plus,
  ArrowUpDown,
  MoreHorizontal,
  Folder,
  Sparkles,
  AlertTriangle,
} from "lucide-vue-next";
import {
  createDraftOrphan,
  listProjects,
  listProjectConversations,
  listOrphanConversations,
} from "../data/projectsStub";
import { useConnectionStatus } from "../composables/useConnectionStatus";

const route = useRoute();
const router = useRouter();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const { statusFor } = useConnectionStatus();

/** 侧栏没有「当前活跃 backend」的概念，按以下规则挑一个显示：
 *  - Claude 配好 → 显示 Claude（与改造前一致）
 *  - Claude 未配但 Codex 配好 → 显示 Codex
 *  - 两者都未配 → 警告未连接
 *  这样不论用户主用哪个 backend 都能在侧栏看到至少一个绿灯。 */
const primaryStatus = computed(() => {
  const claude = statusFor("claude");
  const codex = statusFor("codex");
  if (claude && claude.connectionMode !== "unconfigured") {
    return { backend: "claude" as const, status: claude };
  }
  if (codex && codex.connectionMode !== "unconfigured") {
    return { backend: "codex" as const, status: codex };
  }
  return claude || codex
    ? { backend: "claude" as const, status: claude ?? codex! }
    : null;
});

const backendLabel = computed(() =>
  primaryStatus.value?.backend === "codex" ? "Codex" : "Claude",
);

const isUnconfigured = computed(
  () => primaryStatus.value?.status.connectionMode === "unconfigured" ||
        primaryStatus.value === null,
);

const connectionTooltip = computed(() => {
  const ps = primaryStatus.value;
  if (!ps) return "正在检测 agent 连接…";
  const s = ps.status;
  if (s.connectionMode === "unconfigured") {
    return "CC-Switch 代理不可达。点击进入设置。";
  }
  return `${backendLabel.value} · ${s.effectiveUrl ?? "—"}`;
});

/** 项目树的展开状态，默认展开所有项目（数据少，先粗糙做）。 */
const expanded = reactive<Record<string, boolean>>(
  Object.fromEntries(projects.value.map((p) => [p.id, true])),
);

function toggle(projectId: string) {
  expanded[projectId] = !expanded[projectId];
}

function isActiveTask(projectId: string, taskId: string) {
  return route.path === `/projects/${projectId}/tasks/${taskId}`;
}

function isActiveOrphan(taskId: string) {
  return route.path === `/chats/${taskId}`;
}

/** 点「新对话」：开一条不绑项目的草稿会话，跳到 /chats/:id；
 *  在发出第一条消息之前不会出现在侧栏「零散对话」里。 */
function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

const globalActions = [
  { key: "new-chat", label: "新对话", icon: MessageSquarePlus, handler: newChat },
  { key: "search", label: "搜索", icon: Search, handler: noop },
  { key: "plugins", label: "插件 / 技能", icon: Puzzle, handler: noop },
  { key: "automation", label: "自动化", icon: Zap, handler: noop },
];

function noop() {
  /* 占位：后续接 store / 命令 */
}
</script>

<template>
  <aside class="secondary-panel">
    <!-- 区域 1：全局动作 -->
    <div class="sb-section sb-section--actions">
      <button
        v-for="a in globalActions"
        :key="a.key"
        type="button"
        class="sb-action"
        :title="a.label"
        :aria-label="a.label"
        @click="a.handler"
      >
        <component :is="a.icon" :size="16" aria-hidden="true" />
      </button>
    </div>

    <!-- 区域 2：项目（树状） -->
    <div class="sb-section">
      <div class="sb-section__header">
        <span class="sb-section__title">项目</span>
        <div class="sb-section__tools">
          <button type="button" class="sb-icon-btn" title="添加项目" aria-label="添加项目" @click="noop">
            <Plus :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="sb-icon-btn" title="整理 / 排序" aria-label="整理 / 排序" @click="noop">
            <ArrowUpDown :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="sb-tree">
        <div v-for="p in projects" :key="p.id" class="sb-tree__group">
          <div
            class="sb-tree__row sb-tree__row--project"
            :class="{ 'is-open': expanded[p.id] }"
            role="button"
            tabindex="0"
            :aria-expanded="expanded[p.id]"
            @click="toggle(p.id)"
            @keydown.enter.prevent="toggle(p.id)"
            @keydown.space.prevent="toggle(p.id)"
          >
            <Folder :size="14" aria-hidden="true" />
            <span class="sb-tree__name">{{ p.name }}</span>
            <div class="sb-tree__hover-tools" @click.stop>
              <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话" @click="noop">
                <Plus :size="13" aria-hidden="true" />
              </button>
              <button type="button" class="sb-icon-btn" title="更多" aria-label="更多" @click="noop">
                <MoreHorizontal :size="13" aria-hidden="true" />
              </button>
            </div>
          </div>

          <div
            class="sb-collapse"
            :class="{ 'is-open': expanded[p.id] }"
            :aria-hidden="!expanded[p.id]"
          >
            <div class="sb-collapse__inner">
              <RouterLink
                v-for="c in listProjectConversations(p.id)"
                :key="c.id"
                :to="`/projects/${p.id}/tasks/${c.id}`"
                class="sb-tree__row sb-tree__row--child"
                :class="{ 'is-active': isActiveTask(p.id, c.id) }"
              >
                <span class="sb-tree__name">{{ c.title }}</span>
              </RouterLink>
              <p
                v-if="listProjectConversations(p.id).length === 0"
                class="sb-tree__empty"
              >
                还没有对话
              </p>
            </div>
          </div>
        </div>

        <p v-if="projects.length === 0" class="sb-tree__empty">暂无项目</p>
      </div>
    </div>

    <!-- 区域 3：零散对话 -->
    <div class="sb-section">
      <div class="sb-section__header">
        <span class="sb-section__title">零散对话</span>
        <div class="sb-section__tools">
          <button type="button" class="sb-icon-btn" title="整理 / 排序" aria-label="整理 / 排序" @click="noop">
            <ArrowUpDown :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="sb-tree">
        <RouterLink
          v-for="o in orphans"
          :key="o.id"
          :to="`/chats/${o.id}`"
          class="sb-tree__row sb-tree__row--orphan"
          :class="{ 'is-active': isActiveOrphan(o.id) }"
        >
          <span class="sb-tree__name">{{ o.title }}</span>
        </RouterLink>
        <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
      </div>
    </div>

    <!-- 底部：设置入口 + 连接状态徽章 -->
    <div class="sb-footer">
      <RouterLink
        to="/settings"
        class="sb-footer__btn"
        active-class="is-active"
        title="设置"
        aria-label="设置"
      >
        <Settings :size="16" aria-hidden="true" />
      </RouterLink>

      <RouterLink
        to="/settings"
        class="sb-conn"
        :class="isUnconfigured ? 'sb-conn--warn' : 'sb-conn--ok'"
        :title="connectionTooltip"
        :aria-label="connectionTooltip"
      >
        <template v-if="isUnconfigured">
          <AlertTriangle :size="12" aria-hidden="true" />
          <span class="sb-conn__label">未连接</span>
        </template>
        <template v-else-if="primaryStatus">
          <Sparkles :size="12" aria-hidden="true" />
          <span class="sb-conn__label">{{ backendLabel }}</span>
        </template>
        <template v-else>
          <span class="sb-conn__label sb-conn__label--probing">检测中…</span>
        </template>
      </RouterLink>
    </div>
  </aside>
</template>
