<script setup lang="ts">
import { computed, reactive } from "vue";
import { RouterLink, useRoute } from "vue-router";
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
  listProjects,
  listProjectConversations,
  listOrphanConversations,
} from "../data/projectsStub";
import { useConnectionStatus } from "../composables/useConnectionStatus";

const route = useRoute();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const { status: connStatus } = useConnectionStatus();

/** 侧栏底部那枚徽章上要显示的提供商名。
 *  - cc-switch / direct 给固定友好名；
 *  - custom 从 effectiveUrl 摘 host，便于一眼分辨连到哪里去了；
 *  - unconfigured 不进徽章，会渲染警告小标志。 */
const providerLabel = computed(() => {
  const s = connStatus.value;
  if (!s) return null;
  switch (s.connectionMode) {
    case "cc-switch":
      return "CC-Switch";
    case "direct":
      return "Anthropic";
    case "custom": {
      const url = s.effectiveUrl || "";
      try { return new URL(url).host || "Custom"; }
      catch { return "Custom"; }
    }
    default:
      return null;
  }
});

const isUnconfigured = computed(
  () => connStatus.value?.connectionMode === "unconfigured",
);

const connectionTooltip = computed(() => {
  const s = connStatus.value;
  if (!s) return "正在检测 Claude 连接…";
  if (s.connectionMode === "unconfigured") {
    return "未检测到 Claude 连接：未发现 CC-Switch 本地代理也没有 ANTHROPIC_API_KEY。点击进入设置。";
  }
  return `Claude · ${providerLabel.value}（${s.effectiveUrl ?? "—"}）`;
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

const globalActions = [
  { key: "new-chat", label: "新对话", icon: MessageSquarePlus },
  { key: "search", label: "搜索", icon: Search },
  { key: "plugins", label: "插件 / 技能", icon: Puzzle },
  { key: "automation", label: "自动化", icon: Zap },
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
        @click="noop"
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
        <a
          v-for="o in orphans"
          :key="o.id"
          href="#"
          class="sb-tree__row sb-tree__row--orphan"
          @click.prevent="noop"
        >
          <span class="sb-tree__name">{{ o.title }}</span>
        </a>
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
        <template v-else-if="providerLabel">
          <Sparkles :size="12" aria-hidden="true" />
          <span class="sb-conn__label">Claude · {{ providerLabel }}</span>
        </template>
        <template v-else>
          <span class="sb-conn__label sb-conn__label--probing">检测中…</span>
        </template>
      </RouterLink>
    </div>
  </aside>
</template>
