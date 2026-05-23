<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import {
  Settings,
  MessageSquarePlus,
  Puzzle,
  Plus,
  ChevronsDownUp,
  ChevronsUpDown,
  AlertTriangle,
  FolderOpen,
  FolderPlus,
  Github,
  Sparkles,
  X,
  Archive,
} from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import {
  createProject,
  deriveProjectName,
  listProjects,
} from "../services/projectsStore";
import {
  archiveTask,
  createDraftOrphan,
  createDraftTask,
  listOrphanConversations,
} from "../services/tasksStore";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import { pickFolder } from "../services/projects";

import SidebarSearch from "../components/sidebar/SidebarSearch.vue";
import ProjectTreeItem from "../components/sidebar/ProjectTreeItem.vue";
import CloneRepoDialog from "../components/sidebar/CloneRepoDialog.vue";
import CategoryDialog from "../components/sidebar/CategoryDialog.vue";

const route = useRoute();
const router = useRouter();

const projects = computed(() => listProjects());
const orphans = computed(() => listOrphanConversations());
const { statusFor } = useConnectionStatus();

// ── Connection status badge ──

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

// ── Project tree expansion ──

const expanded = reactive<Record<string, boolean>>(
  Object.fromEntries(projects.value.map((p) => [p.id, true])),
);

function toggle(projectId: string) {
  expanded[projectId] = !expanded[projectId];
}

const allExpanded = computed(
  () => projects.value.length > 0 && projects.value.every((p) => expanded[p.id]),
);

function toggleAll() {
  const target = !allExpanded.value;
  for (const p of projects.value) expanded[p.id] = target;
}

// ── Orphan inbox ──

const orphansExpanded = ref(true);
function toggleOrphans() {
  orphansExpanded.value = !orphansExpanded.value;
}

// --- 孤儿 session 归档确认 ---
const confirmingOrphanId = ref<string | null>(null);

function onOrphanArchiveClick(e: MouseEvent, orphanId: string) {
  e.preventDefault();
  e.stopPropagation();
  if (confirmingOrphanId.value === orphanId) {
    archiveTask(orphanId);
    confirmingOrphanId.value = null;
  } else {
    confirmingOrphanId.value = orphanId;
  }
}

function onOrphanRowLeave() {
  confirmingOrphanId.value = null;
}

// ── Navigation helpers ──

function isActiveOrphan(taskId: string) {
  return route.path === `/chats/${taskId}`;
}

// ── New chat ──

function newChat() {
  const draft = createDraftOrphan();
  router.push(`/chats/${draft.id}`);
}

function newProjectChat(projectId: string) {
  const draft = createDraftTask(projectId);
  if (!draft) return;
  expanded[projectId] = true;
  router.push(`/projects/${projectId}/tasks/${draft.id}`);
}

function onSearchSelect(r: { route: string }) {
  router.push(r.route);
}

// ── Add project menu ──

const addMenuOpen = ref(false);
const menuPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const MENU_W = 200;
const MENU_H_EST = 132;
const projectError = ref<string | null>(null);

function dismissError() {
  projectError.value = null;
}

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

// ── Local folder picker ──

async function pickLocalFolder() {
  closeAddMenu();
  projectError.value = null;
  try {
    const picked = await pickFolder({ title: "选择项目根目录" });
    if (!picked) return;
    const project = await createProject({
      name: deriveProjectName(picked) || "新项目",
      cwd: picked,
    });
    expanded[project.id] = true;
  } catch (err) {
    projectError.value = `选择文件夹失败：${String(err)}`;
  }
}

// ── Clone dialog ──

const cloneOpen = ref(false);
const cloneDialogRef = ref<InstanceType<typeof CloneRepoDialog> | null>(null);

function openClone() {
  closeAddMenu();
  cloneOpen.value = true;
  nextTick(() => cloneDialogRef.value?.init());
}

function onCloneCreated(p: Project) {
  expanded[p.id] = true;
}

// ── Category dialog ──

const categoryOpen = ref(false);
const categoryDialogRef = ref<InstanceType<typeof CategoryDialog> | null>(null);

function openCategory() {
  closeAddMenu();
  categoryOpen.value = true;
  nextTick(() => categoryDialogRef.value?.init());
}

function onCategoryCreated(p: Project) {
  expanded[p.id] = true;
}

// ── Project tree item handlers ──

function onProjectArchived() {
  router.push("/");
}

function onProjectDeleted(projectId: string) {
  router.push("/");
  delete expanded[projectId];
}
</script>

<template>
  <aside class="secondary-panel">
    <!-- Actions: new chat + search -->
    <div class="sb-section sb-section--actions">
      <button type="button" class="sb-primary-btn" title="新对话" aria-label="新对话" @click="newChat">
        <MessageSquarePlus :size="15" aria-hidden="true" />
        <span class="sb-primary-btn__label">新对话</span>
      </button>
      <SidebarSearch @select="onSearchSelect" />
    </div>

    <!-- Project tree -->
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
        <ProjectTreeItem
          v-for="p in projects"
          :key="p.id"
          :project="p"
          :is-expanded="expanded[p.id]"
          @toggle="toggle"
          @new-chat="newProjectChat"
          @error="(msg: string) => projectError = msg"
          @archived="onProjectArchived"
          @deleted="onProjectDeleted(p.id)"
        />

        <p v-if="projects.length === 0" class="sb-tree__empty">暂无项目</p>
      </div>
    </div>

    <!-- Orphan inbox -->
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
              :class="{ 'is-active': isActiveOrphan(o.id) }"
              @mouseleave="onOrphanRowLeave">
              <span class="sb-tree__name">{{ o.title }}</span>
              <div class="sb-tree__hover-tools" @click.stop>
                <button type="button" class="sb-icon-btn" :class="{ 'is-confirming': confirmingOrphanId === o.id }"
                  :title="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                  :aria-label="confirmingOrphanId === o.id ? '确认归档，再点一次' : '归档'"
                  @click="onOrphanArchiveClick($event, o.id)">
                  <template v-if="confirmingOrphanId === o.id">确认</template>
                  <Archive v-else :size="13" aria-hidden="true" />
                </button>
              </div>
            </RouterLink>
            <p v-if="orphans.length === 0" class="sb-tree__empty">没有未绑定的对话</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Footer -->
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

    <!-- Add project context menu -->
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

    <!-- Clone dialog -->
    <CloneRepoDialog
      ref="cloneDialogRef"
      :open="cloneOpen"
      @close="cloneOpen = false"
      @cloned="onCloneCreated"
      @error="(msg: string) => projectError = msg"
    />

    <!-- Category dialog -->
    <CategoryDialog
      ref="categoryDialogRef"
      :open="categoryOpen"
      @close="categoryOpen = false"
      @created="onCategoryCreated"
    />
  </aside>
</template>
