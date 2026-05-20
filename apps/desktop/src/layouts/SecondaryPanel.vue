<script setup lang="ts">
import { computed, reactive } from "vue";
import { RouterLink, useRoute } from "vue-router";
import {
  Cog,
  Plug,
  Info,
  MessageSquarePlus,
  Search,
  Puzzle,
  Zap,
  Plus,
  ArrowUpDown,
  MoreHorizontal,
  Folder,
  MessageSquare,
} from "lucide-vue-next";
import {
  listProjects,
  listProjectConversations,
  listOrphanConversations,
} from "../data/projectsStub";

interface Props {
  section: "projects" | "settings";
}
const props = defineProps<Props>();
const route = useRoute();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());

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

const settingsNav = [
  { to: "/settings", label: "通用", icon: Cog },
  { to: "/settings", label: "Claude CLI", icon: Plug },
  { to: "/settings", label: "关于", icon: Info },
];

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
    <template v-if="props.section === 'projects'">
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
                  <MessageSquare :size="13" aria-hidden="true" />
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
            <MessageSquare :size="13" aria-hidden="true" />
            <span class="sb-tree__name">{{ o.title }}</span>
          </a>
          <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="sb-section">
        <div class="sb-section__header">
          <span class="sb-section__title">设置</span>
        </div>
        <nav class="secondary-panel__nav">
          <RouterLink
            v-for="item in settingsNav"
            :key="item.label"
            :to="item.to"
            class="secondary-panel__item"
            active-class="is-active"
          >
            <component :is="item.icon" :size="16" aria-hidden="true" />
            {{ item.label }}
          </RouterLink>
        </nav>
      </div>
    </template>
  </aside>
</template>
