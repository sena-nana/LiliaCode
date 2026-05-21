<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Settings,
  MessageSquarePlus,
  Search,
  Puzzle,
  Plus,
  ArrowUpDown,
  MoreHorizontal,
  Folder,
  Sparkles,
  AlertTriangle,
  FileText,
  X,
} from "lucide-vue-next";
import {
  createDraftOrphan,
  listProjects,
  listProjectConversations,
  listOrphanConversations,
} from "../data/projectsStub";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { searchSessions, type SearchResult } from "../services/sessionSearch";

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

function noop() {
  /* 占位：后续接 store / 命令 */
}

// ---------------- 内联搜索 ---------------- *
// 「新对话」按钮和「搜索」按钮独占第一行；点击搜索后这两个按钮原位变成一个
// 搜索输入框 + 关闭按钮，下方挂下拉。下拉里的结果走 hybrid 模式（文本子串
// 命中优先，向量相似度兜底召回）。键盘：↑↓ 选项，Enter 打开，Esc 关闭。

const searchActive = ref(false);
const searchQuery = ref("");
const searchInput = ref<HTMLInputElement | null>(null);
const selectedIdx = ref(0);

const searchResults = computed<SearchResult[]>(() =>
  searchSessions(searchQuery.value, "hybrid").slice(0, 12),
);

watch(searchResults, () => {
  selectedIdx.value = 0;
});

async function openSearch() {
  searchActive.value = true;
  searchQuery.value = "";
  selectedIdx.value = 0;
  await nextTick();
  searchInput.value?.focus();
}

function closeSearch() {
  searchActive.value = false;
  searchQuery.value = "";
  selectedIdx.value = 0;
}

function openResult(r: SearchResult) {
  router.push(r.route);
  closeSearch();
}

function onSearchKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    closeSearch();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (searchResults.value.length) {
      selectedIdx.value = (selectedIdx.value + 1) % searchResults.value.length;
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (searchResults.value.length) {
      selectedIdx.value =
        (selectedIdx.value - 1 + searchResults.value.length) %
        searchResults.value.length;
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    const r = searchResults.value[selectedIdx.value];
    if (r) openResult(r);
  }
}

interface Segment {
  text: string;
  mark: boolean;
}

/** 把高亮 ranges 转成「文本段 + 是否高亮」的扁平数组，方便模板渲染。 */
function highlightSegments(text: string, ranges: Array<[number, number]>): Segment[] {
  if (!ranges.length) return [{ text, mark: false }];
  const sorted = [...ranges].sort((a, b) => a[0] - b[0]);
  const merged: Array<[number, number]> = [];
  for (const [s, e] of sorted) {
    const last = merged[merged.length - 1];
    if (last && s <= last[1]) last[1] = Math.max(last[1], e);
    else merged.push([s, e]);
  }
  const out: Segment[] = [];
  let cur = 0;
  for (const [s, e] of merged) {
    if (cur < s) out.push({ text: text.slice(cur, s), mark: false });
    out.push({ text: text.slice(s, e), mark: true });
    cur = e;
  }
  if (cur < text.length) out.push({ text: text.slice(cur), mark: false });
  return out;
}
</script>

<template>
  <aside class="secondary-panel">
    <!-- 区域 1：第一行 = 新对话（宽）+ 搜索（图标）。点击搜索后整行变输入框。
         搜索下拉作为 actions 的子元素，absolute 浮在项目树之上，不挤占布局。 -->
    <div class="sb-section sb-section--actions">
      <template v-if="!searchActive">
        <button type="button" class="sb-primary-btn" title="新对话" aria-label="新对话" @click="newChat">
          <MessageSquarePlus :size="15" aria-hidden="true" />
          <span class="sb-primary-btn__label">新对话</span>
        </button>
        <button type="button" class="sb-icon-action" title="搜索会话" aria-label="搜索会话" @click="openSearch">
          <Search :size="15" aria-hidden="true" />
        </button>
      </template>

      <template v-else>
        <div class="sb-search">
          <Search :size="14" aria-hidden="true" class="sb-search__leading" />
          <input ref="searchInput" v-model="searchQuery" type="text" class="sb-search__input" placeholder="搜索会话…"
            spellcheck="false" @keydown="onSearchKeydown" />
        </div>
        <button type="button" class="sb-icon-action" title="关闭搜索 (Esc)" aria-label="关闭搜索" @click="closeSearch">
          <X :size="15" aria-hidden="true" />
        </button>

        <div class="sb-search-dd" role="listbox">
          <template v-if="searchResults.length">
            <button v-for="(r, i) in searchResults" :key="r.route" type="button" class="sb-search-dd__item"
              :class="{ 'is-active': i === selectedIdx }" role="option" :aria-selected="i === selectedIdx"
              @mouseenter="selectedIdx = i" @click="openResult(r)">
              <span class="sb-search-dd__title">
                <template v-for="(seg, j) in highlightSegments(r.title, r.highlights)" :key="j">
                  <mark v-if="seg.mark">{{ seg.text }}</mark>
                  <template v-else>{{ seg.text }}</template>
                </template>
              </span>
              <span v-if="r.projectName" class="sb-search-dd__scope">{{ r.projectName }}</span>
            </button>
          </template>
          <p v-else-if="searchQuery.trim()" class="sb-search-dd__empty">没有匹配</p>
          <p v-else class="sb-search-dd__hint">
            <FileText :size="11" aria-hidden="true" />
            输入关键词
          </p>
        </div>
      </template>
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
          <div class="sb-tree__row sb-tree__row--project" :class="{ 'is-open': expanded[p.id] }" role="button"
            tabindex="0" :aria-expanded="expanded[p.id]" @click="toggle(p.id)" @keydown.enter.prevent="toggle(p.id)"
            @keydown.space.prevent="toggle(p.id)">
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

          <div class="sb-collapse" :class="{ 'is-open': expanded[p.id] }" :aria-hidden="!expanded[p.id]">
            <div class="sb-collapse__inner">
              <RouterLink v-for="c in listProjectConversations(p.id)" :key="c.id"
                :to="`/projects/${p.id}/tasks/${c.id}`" class="sb-tree__row sb-tree__row--child"
                :class="{ 'is-active': isActiveTask(p.id, c.id) }">
                <span class="sb-tree__name">{{ c.title }}</span>
              </RouterLink>
              <p v-if="listProjectConversations(p.id).length === 0" class="sb-tree__empty">
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
        <RouterLink v-for="o in orphans" :key="o.id" :to="`/chats/${o.id}`" class="sb-tree__row sb-tree__row--orphan"
          :class="{ 'is-active': isActiveOrphan(o.id) }">
          <span class="sb-tree__name">{{ o.title }}</span>
        </RouterLink>
        <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
      </div>
    </div>

    <!-- 底部：设置 + 插件 + 连接状态徽章 -->
    <div class="sb-footer">
      <RouterLink to="/settings" class="sb-footer__btn" active-class="is-active" title="设置" aria-label="设置">
        <Settings :size="14" aria-hidden="true" />
      </RouterLink>

      <RouterLink to="/plugins" class="sb-footer__btn" active-class="is-active" title="插件 / 技能" aria-label="插件 / 技能">
        <Puzzle :size="14" aria-hidden="true" />
      </RouterLink>

      <RouterLink to="/settings" class="sb-conn" :class="isUnconfigured ? 'sb-conn--warn' : 'sb-conn--ok'"
        :title="connectionTooltip" :aria-label="connectionTooltip">
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