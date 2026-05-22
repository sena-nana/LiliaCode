<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Settings,
  MessageSquarePlus,
  Search,
  Puzzle,
  Plus,
  ChevronsDownUp,
  ChevronsUpDown,
  MoreHorizontal,
  Folder,
  FolderOpen,
  FolderPlus,
  Github,
  Sparkles,
  AlertTriangle,
  FileText,
  X,
  Pencil,
  Archive,
  Trash2,
  Code2,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { vContextMenu } from "../directives/contextMenu";
import {
  openContextMenuAt,
  type ContextMenuItem,
} from "../composables/useContextMenu";
import { homeDir } from "@tauri-apps/api/path";
import {
  archiveProjectConversations,
  createProject,
  deriveProjectName,
  listProjects,
  removeProject,
  renameProject,
} from "../services/projectsStore";
import {
  createDraftOrphan,
  createDraftTask,
  listProjectConversations,
  listOrphanConversations,
} from "../services/tasksStore";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { searchSessions, type SearchResult } from "../services/sessionSearch";
import {
  getProjectSettings,
  gitCloneRepo,
  openInFileManager,
  openInVSCode,
  pickFolder,
} from "../services/projects";

const route = useRoute();
const router = useRouter();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const { statusFor } = useConnectionStatus();

/** 侧栏挑一个 backend 显示连接状态：Claude 优先，Codex 兜底，都未配则警告。 */
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

/** 项目树展开状态，默认全部展开。 */
const expanded = reactive<Record<string, boolean>>(
  Object.fromEntries(projects.value.map((p) => [p.id, true])),
);

function toggle(projectId: string) {
  expanded[projectId] = !expanded[projectId];
}

/** 是否全部展开：空列表时返回 false，让按钮以「展开」状态登场。 */
const allExpanded = computed(
  () => projects.value.length > 0 && projects.value.every((p) => expanded[p.id]),
);

function toggleAll() {
  const target = !allExpanded.value;
  for (const p of projects.value) expanded[p.id] = target;
}

/** 收集箱整段展开/折叠：展开就显示孤儿列表，折叠就只留标题栏。 */
const orphansExpanded = ref(true);
function toggleOrphans() {
  orphansExpanded.value = !orphansExpanded.value;
}

function isActiveTask(projectId: string, taskId: string) {
  return route.path === `/projects/${projectId}/tasks/${taskId}`;
}

function isActiveOrphan(taskId: string) {
  return route.path === `/chats/${taskId}`;
}

/** 点「新对话」：开一条不绑项目的草稿会话；第一条消息发出去之前不进侧栏。 */
function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

/** 点项目行右侧 + ：在该项目下开一条草稿任务，首条消息发出前不进项目子树。 */
function newProjectChat(projectId: string) {
  const draft = createDraftTask(projectId);
  if (!draft) return;
  expanded[projectId] = true;
  router.push(`/projects/${projectId}/tasks/${draft.id}`);
}

// ---------------- 内联搜索 ---------------- *
// 点击搜索后「新对话 + 搜索」按钮原位变成「输入框 + 关闭按钮」，下方挂下拉。
// 下拉走 hybrid 模式（文本子串命中优先，向量相似度兜底召回）。键盘：↑↓ 选项，Enter 打开，Esc 关闭。

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

// ---------------- 添加项目 ---------------- *
// 「+」按钮点开下拉小菜单：本地文件夹 / GitHub clone / 空分类。
// 菜单走「鼠标右下角」原生 contextmenu 模式：Teleport 到 body、position:fixed，
// 锚点取触发点的 clientX/clientY，再 clamp 在视口内避免穿底/穿右。

const addMenuOpen = ref(false);
const menuPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const MENU_W = 200;
const MENU_H_EST = 132;

function openAddMenu(e: MouseEvent) {
  const x = Math.min(e.clientX, window.innerWidth - MENU_W - 4);
  const y = Math.min(e.clientY, window.innerHeight - MENU_H_EST - 4);
  menuPos.value = { x: Math.max(4, x), y: Math.max(4, y) };
  addMenuOpen.value = true;
}

function closeAddMenu() {
  addMenuOpen.value = false;
}

function onDocPointer(e: PointerEvent) {
  // Teleport 到 body 后菜单 DOM 在外面，直接看点中元素有没有 .sb-menu 祖先。
  const target = e.target as HTMLElement | null;
  if (target && target.closest && target.closest(".sb-menu")) return;
  closeAddMenu();
}

function onDocKey(e: KeyboardEvent) {
  if (e.key === "Escape" && addMenuOpen.value) {
    closeAddMenu();
    e.stopPropagation();
  }
}

watch(addMenuOpen, async (v) => {
  if (v) {
    await nextTick();
    document.addEventListener("pointerdown", onDocPointer, true);
    document.addEventListener("keydown", onDocKey);
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onDocKey);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointer, true);
  document.removeEventListener("keydown", onDocKey);
});

const projectError = ref<string | null>(null);

function dismissError() {
  projectError.value = null;
}

async function pickLocalFolder() {
  closeAddMenu();
  projectError.value = null;
  try {
    const picked = await pickFolder({ title: "选择项目根目录" });
    if (!picked) return;
    const project = createProject({
      name: deriveProjectName(picked) || "新项目",
      cwd: picked,
    });
    expanded[project.id] = true;
  } catch (err) {
    projectError.value = `选择文件夹失败：${String(err)}`;
  }
}

// ----- GitHub clone dialog -----
const cloneOpen = ref(false);
const cloneUrl = ref("");
const cloneParent = ref<string>("");
const cloneInput = ref<HTMLInputElement | null>(null);
const cloneBusy = ref(false);
const cloneError = ref<string | null>(null);

async function openClone() {
  closeAddMenu();
  cloneUrl.value = "";
  cloneError.value = null;
  cloneBusy.value = false;
  // 父目录：Settings 里的偏好 → home 兜底。
  try {
    const s = await getProjectSettings();
    if (s.cloneParentDir && s.cloneParentDir.trim()) {
      cloneParent.value = s.cloneParentDir.trim();
    } else {
      cloneParent.value = await safeHomeDir();
    }
  } catch {
    cloneParent.value = await safeHomeDir();
  }
  cloneOpen.value = true;
  await nextTick();
  cloneInput.value?.focus();
}

async function safeHomeDir(): Promise<string> {
  try { return await homeDir(); }
  catch { return ""; }
}

async function pickCloneParent() {
  try {
    const picked = await pickFolder({
      title: "选择 clone 目标父目录",
      defaultPath: cloneParent.value || null,
    });
    if (picked) cloneParent.value = picked;
  } catch (err) {
    cloneError.value = `选择文件夹失败：${String(err)}`;
  }
}

/** 给用户预览「最终会克隆到哪里」，与 Rust 端的推断保持一致。 */
const cloneTargetPreview = computed(() => {
  const url = cloneUrl.value.trim();
  const parent = cloneParent.value.trim();
  if (!url || !parent) return "";
  const cleaned = url.replace(/\.git$/i, "").replace(/\/+$/, "");
  const base = cleaned.split(/[/:]/).pop()?.trim() || "repo";
  const sep = parent.includes("\\") ? "\\" : "/";
  const normalizedParent = parent.replace(/[\\/]+$/, "");
  return `${normalizedParent}${sep}${base}`;
});

async function confirmClone() {
  if (cloneBusy.value) return;
  cloneError.value = null;
  const url = cloneUrl.value.trim();
  const parent = cloneParent.value.trim();
  if (!url) {
    cloneError.value = "请填写仓库 URL";
    return;
  }
  if (!parent) {
    cloneError.value = "请选择 clone 目标父目录";
    return;
  }
  cloneBusy.value = true;
  try {
    const cloned = await gitCloneRepo(url, parent);
    const project = createProject({
      name: deriveProjectName(cloned) || "新项目",
      cwd: cloned,
    });
    expanded[project.id] = true;
    cloneOpen.value = false;
  } catch (err) {
    cloneError.value = String(err);
  } finally {
    cloneBusy.value = false;
  }
}

// ----- 空分类 dialog -----
const categoryOpen = ref(false);
const categoryName = ref("");
const categoryInput = ref<HTMLInputElement | null>(null);

async function openCategory() {
  closeAddMenu();
  categoryName.value = "";
  categoryOpen.value = true;
  await nextTick();
  categoryInput.value?.focus();
}

function confirmCategory() {
  const name = categoryName.value.trim();
  if (!name) return;
  const project = createProject({ name, cwd: null });
  expanded[project.id] = true;
  categoryOpen.value = false;
}

// ---------------- 项目行：更多按钮 / 右键菜单 ---------------- *
// 同一组菜单服务两种入口：右键走 v-context-menu 指令；左键点 ⋯ 走 openContextMenuAt。
// 菜单项里两条「文件管理器 / VSCode」对仅作分类用的项目（cwd === null）禁用。

const editingId = ref<string | null>(null);
const editingValue = ref("");
const editingInput = ref<HTMLInputElement | null>(null);

function reportError(msg: string) {
  projectError.value = msg;
}

async function openInExplorer(p: Project) {
  if (!p.cwd) return;
  try {
    await openInFileManager(p.cwd);
  } catch (err) {
    reportError(`在资源管理器中打开失败：${String(err)}`);
  }
}

async function openWithVSCode(p: Project) {
  if (!p.cwd) return;
  try {
    await openInVSCode(p.cwd);
  } catch (err) {
    reportError(`用 VSCode 打开失败：${String(err)}`);
  }
}

async function startRename(p: Project) {
  editingId.value = p.id;
  editingValue.value = p.name;
  await nextTick();
  editingInput.value?.focus();
  editingInput.value?.select();
}

function commitRename() {
  const id = editingId.value;
  if (!id) return;
  const next = editingValue.value.trim();
  if (next) renameProject(id, next);
  editingId.value = null;
  editingValue.value = "";
}

function cancelRename() {
  editingId.value = null;
  editingValue.value = "";
}

/** 编辑态键盘：Enter 提交、Esc 取消，其它键都 stop 一下避免冒泡触发行级 toggle。 */
function onEditingKeydown(e: KeyboardEvent) {
  e.stopPropagation();
  if (e.key === "Enter") {
    e.preventDefault();
    commitRename();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cancelRename();
  }
}

/** 函数 ref：input 在 v-for 里，普通 ref 会被收集成数组；用函数 ref 直接绑到模块 ref 上。 */
function bindEditingInput(el: unknown) {
  editingInput.value = (el as HTMLInputElement | null) ?? null;
}

function archiveAllConversations(p: Project) {
  archiveProjectConversations(p.id);
  // 当前活动路由如果就在被归档的项目下，跳回主页（router.push("/")），免得停在不存在的 task。
  if (
    route.params.projectId &&
    String(route.params.projectId) === p.id
  ) {
    router.push("/");
  }
}

function deleteProject(p: Project) {
  removeProject(p.id);
  if (
    route.params.projectId &&
    String(route.params.projectId) === p.id
  ) {
    router.push("/");
  }
  // 顺手清掉 expanded 里的孤儿条目，免得日后同 id 复用拿到旧状态。
  delete expanded[p.id];
}

function buildProjectMenu(p: Project): ContextMenuItem[] {
  const hasCwd = !!p.cwd;
  return [
    {
      id: "open-explorer",
      label: "在资源管理器中打开",
      icon: FolderOpen,
      disabled: !hasCwd,
      onSelect: () => openInExplorer(p),
    },
    {
      id: "open-vscode",
      label: "在 VSCode 中打开",
      icon: Code2,
      disabled: !hasCwd,
      onSelect: () => openWithVSCode(p),
    },
    {
      id: "rename",
      label: "重命名项目",
      icon: Pencil,
      onSelect: () => startRename(p),
    },
    {
      id: "archive",
      label: "归档所有对话",
      icon: Archive,
      confirmLabel: "确认归档？再点一次",
      onSelect: () => archiveAllConversations(p),
    },
    {
      id: "remove",
      label: "移除项目",
      icon: Trash2,
      danger: true,
      confirmLabel: "确认移除？再点一次",
      onSelect: () => deleteProject(p),
    },
  ];
}

function onMoreClick(e: MouseEvent, p: Project) {
  e.stopPropagation();
  const btn = e.currentTarget as HTMLElement | null;
  // 锚点取按钮右下角，鼠标点偏一两像素也保持菜单位置稳定；ContextMenuHost 会 clamp 视口。
  if (btn) {
    const rect = btn.getBoundingClientRect();
    openContextMenuAt(rect.right, rect.bottom + 2, buildProjectMenu(p));
  } else {
    openContextMenuAt(e.clientX, e.clientY, buildProjectMenu(p));
  }
}
</script>

<template>
  <aside class="secondary-panel">
    <!-- 区域 1：新对话（宽）+ 搜索（图标）。点击搜索后整行变输入框，下拉浮在树之上。 -->
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
          <button type="button" class="sb-icon-btn"
            :title="allExpanded ? '全部折叠' : '全部展开'"
            :aria-label="allExpanded ? '全部折叠' : '全部展开'"
            @click="toggleAll">
            <ChevronsDownUp v-if="allExpanded" :size="14" aria-hidden="true" />
            <ChevronsUpDown v-else :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="sb-icon-btn" title="添加项目" aria-label="添加项目"
            :aria-expanded="addMenuOpen" :aria-haspopup="true"
            @click="openAddMenu">
            <Plus :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div v-if="projectError" class="sb-banner sb-banner--err">
        <AlertTriangle :size="12" aria-hidden="true" />
        <span class="sb-banner__msg">{{ projectError }}</span>
        <button type="button" class="sb-icon-btn" @click="dismissError" aria-label="忽略错误">
          <X :size="12" aria-hidden="true" />
        </button>
      </div>

      <div class="sb-tree">
        <div v-for="p in projects" :key="p.id" class="sb-tree__group">
          <div class="sb-tree__row sb-tree__row--project"
            :class="{ 'is-open': expanded[p.id], 'is-editing': editingId === p.id }"
            :role="editingId === p.id ? undefined : 'button'"
            :tabindex="editingId === p.id ? -1 : 0"
            :aria-expanded="expanded[p.id]"
            v-context-menu="() => buildProjectMenu(p)"
            @click="editingId === p.id ? null : toggle(p.id)"
            @keydown.enter.prevent="editingId === p.id ? null : toggle(p.id)"
            @keydown.space.prevent="editingId === p.id ? null : toggle(p.id)">
            <Folder :size="14" aria-hidden="true" />
            <input
              v-if="editingId === p.id"
              :ref="bindEditingInput"
              v-model="editingValue"
              type="text"
              class="sb-tree__rename-input"
              spellcheck="false"
              @click.stop
              @pointerdown.stop
              @keydown="onEditingKeydown"
              @blur="commitRename"
            />
            <span v-else class="sb-tree__name">{{ p.name }}</span>
            <div v-if="editingId !== p.id" class="sb-tree__hover-tools" @click.stop>
              <button type="button" class="sb-icon-btn" title="更多" aria-label="更多" @click="onMoreClick($event, p)">
                <MoreHorizontal :size="13" aria-hidden="true" />
              </button>
              <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话" @click="newProjectChat(p.id)">
                <Plus :size="13" aria-hidden="true" />
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

    <!-- 区域 3：收集箱 -->
    <div class="sb-section">
      <div class="sb-section__header">
        <span class="sb-section__title">收集箱</span>
        <div class="sb-section__tools">
          <button type="button" class="sb-icon-btn"
            :title="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
            :aria-label="orphansExpanded ? '折叠收集箱' : '展开收集箱'"
            @click="toggleOrphans">
            <ChevronsDownUp v-if="orphansExpanded" :size="14" aria-hidden="true" />
            <ChevronsUpDown v-else :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="sb-icon-btn" title="新对话" aria-label="新对话" @click="newChat">
            <Plus :size="14" aria-hidden="true" />
          </button>
        </div>
      </div>

      <div class="sb-collapse" :class="{ 'is-open': orphansExpanded }" :aria-hidden="!orphansExpanded">
        <div class="sb-collapse__inner">
          <div class="sb-tree">
            <RouterLink v-for="o in orphans" :key="o.id" :to="`/chats/${o.id}`" class="sb-tree__row sb-tree__row--orphan"
              :class="{ 'is-active': isActiveOrphan(o.id) }">
              <span class="sb-tree__name">{{ o.title }}</span>
            </RouterLink>
            <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
          </div>
        </div>
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

    <!-- ===== 添加项目 contextmenu =====
         Teleport 到 body + position:fixed，相对鼠标点击点定位。 -->
    <Teleport to="body">
      <div v-if="addMenuOpen" class="sb-menu" role="menu"
        :style="{ left: `${menuPos.x}px`, top: `${menuPos.y}px` }">
        <button type="button" class="sb-menu__item" role="menuitem" @click="pickLocalFolder">
          <FolderOpen :size="13" aria-hidden="true" />
          <span class="sb-menu__label">使用本地文件夹</span>
        </button>
        <button type="button" class="sb-menu__item" role="menuitem" @click="openClone">
          <Github :size="13" aria-hidden="true" />
          <span class="sb-menu__label">从 GitHub clone</span>
        </button>
        <button type="button" class="sb-menu__item" role="menuitem" @click="openCategory">
          <FolderPlus :size="13" aria-hidden="true" />
          <span class="sb-menu__label">创建空分类</span>
        </button>
      </div>
    </Teleport>

    <!-- ===== Clone 弹层 ===== -->
    <Teleport to="body">
      <Transition name="search-palette">
        <div v-if="cloneOpen" class="search-palette" role="dialog" aria-modal="true" aria-label="从 GitHub clone"
          @click.self="cloneOpen = false">
          <div class="search-palette__card dialog__card">
            <div class="dialog__header">
              <Github :size="14" aria-hidden="true" />
              <span>从 Git 仓库克隆</span>
            </div>
            <div class="dialog__body">
              <label>
                <span>仓库 URL</span>
                <input ref="cloneInput" v-model="cloneUrl" type="text" class="text-input"
                  placeholder="https://github.com/owner/repo.git"
                  @keydown.enter.prevent="confirmClone" />
              </label>
              <label>
                <span>目标父目录</span>
                <div class="dialog__field-row">
                  <input :value="cloneParent" type="text" class="text-input"
                    placeholder="选择克隆到哪个目录下" readonly />
                  <button type="button" class="ghost" :disabled="cloneBusy" @click="pickCloneParent">
                    <FolderOpen :size="12" aria-hidden="true" /> 选择
                  </button>
                </div>
              </label>
              <p v-if="cloneTargetPreview" class="plugins-create__hint">
                将克隆到 <code>{{ cloneTargetPreview }}</code>
              </p>
              <p v-if="cloneError" class="plugins-create__error">{{ cloneError }}</p>
            </div>
            <div class="dialog__actions">
              <button type="button" class="ghost" :disabled="cloneBusy" @click="cloneOpen = false">取消</button>
              <button type="button" class="primary" :disabled="cloneBusy" @click="confirmClone">
                {{ cloneBusy ? "克隆中…" : "克隆并添加" }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- ===== 空分类弹层 ===== -->
    <Teleport to="body">
      <Transition name="search-palette">
        <div v-if="categoryOpen" class="search-palette" role="dialog" aria-modal="true" aria-label="创建空分类"
          @click.self="categoryOpen = false">
          <div class="search-palette__card dialog__card">
            <div class="dialog__header">
              <FolderPlus :size="14" aria-hidden="true" />
              <span>创建空分类</span>
            </div>
            <div class="dialog__body">
              <label>
                <span>分类名称</span>
                <input ref="categoryInput" v-model="categoryName" type="text" class="text-input"
                  placeholder="例如：实验、归档…"
                  @keydown.enter.prevent="confirmCategory" />
              </label>
              <p class="plugins-create__hint">
                空分类不绑定本地目录，只用来在侧栏里把收集箱里的对话归到一起。
              </p>
            </div>
            <div class="dialog__actions">
              <button type="button" class="ghost" @click="categoryOpen = false">取消</button>
              <button type="button" class="primary" :disabled="!categoryName.trim()" @click="confirmCategory">
                创建
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </aside>
</template>
