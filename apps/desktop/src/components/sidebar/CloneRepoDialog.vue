<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { FolderOpen, Github, Lock, Search, Sparkles } from "lucide-vue-next";
import type {
  GitHubBindingStatus,
  GitHubRepoSummary,
  Project,
} from "@lilia/contracts";
import {
  createProject,
  deriveProjectName,
} from "../../services/projectsStore";
import {
  getGitHubBindingStatus,
  getProjectSettings,
  gitCloneRepo,
  gitHubCloneRepo,
  listGitHubRepos,
  pickFolder,
} from "../../services/projects";

const emit = defineEmits<{
  close: [];
  cloned: [project: Project];
  error: [msg: string];
}>();

const cloneInput = ref<HTMLInputElement | null>(null);
const cloneBusy = ref(false);
const cloneError = ref<string | null>(null);
const cloneParent = ref("");
const query = ref("");
const bindingStatus = ref<GitHubBindingStatus | null>(null);
const repoItems = ref<GitHubRepoSummary[]>([]);
const repoDropdownOpen = ref(false);
const repoLoading = ref(false);
const repoLoadingMore = ref(false);
const nextRepoPage = ref<number | null>(null);
const selectedRepo = ref<GitHubRepoSummary | null>(null);

const isBound = computed(() => bindingStatus.value?.state === "bound");
const queryTrimmed = computed(() => query.value.trim());

function normalizeGitHubInput(input: string): string | null {
  const trimmed = input.trim().replace(/\/+$/, "");
  if (!trimmed) return null;
  const matched = trimmed.match(/^https?:\/\/github\.com\/([^/\s]+)\/([^/\s]+?)(?:\.git)?$/i);
  if (matched) return `${matched[1]}/${matched[2]}`;
  if (/^[^/\s]+\/[^/\s]+$/.test(trimmed)) return trimmed.replace(/\.git$/i, "");
  return null;
}

function dedupeRepos(items: GitHubRepoSummary[]): GitHubRepoSummary[] {
  const seen = new Set<string>();
  const next: GitHubRepoSummary[] = [];
  for (const item of items) {
    if (seen.has(item.fullName)) continue;
    seen.add(item.fullName);
    next.push(item);
  }
  return next;
}

const filteredRepos = computed(() => {
  const text = queryTrimmed.value.toLowerCase();
  if (!text) return repoItems.value;
  return repoItems.value.filter((repo) =>
    repo.fullName.toLowerCase().includes(text) ||
    repo.name.toLowerCase().includes(text) ||
    (repo.description ?? "").toLowerCase().includes(text)
  );
});

const directCloneRepo = computed(() => normalizeGitHubInput(queryTrimmed.value));

const cloneTargetPreview = computed(() => {
  const parent = cloneParent.value.trim();
  if (!parent) return "";
  const baseName = selectedRepo.value?.name ||
    (() => {
      const githubRepo = normalizeGitHubInput(queryTrimmed.value);
      if (githubRepo) return githubRepo.split("/").pop() ?? "repo";
      const https = queryTrimmed.value.replace(/\.git$/i, "").replace(/\/+$/, "");
      return https ? (https.split(/[/:]/).pop()?.trim() || "repo") : "";
    })();
  if (!baseName) return "";
  const sep = parent.includes("\\") ? "\\" : "/";
  const normalizedParent = parent.replace(/[\\/]+$/, "");
  return `${normalizedParent}${sep}${baseName}`;
});

async function safeHomeDir(): Promise<string> {
  try {
    const { homeDir } = await import("@tauri-apps/api/path");
    return await homeDir();
  } catch {
    return "";
  }
}

async function loadBindingStatus() {
  bindingStatus.value = await getGitHubBindingStatus();
}

async function loadRepoPage(page = 1, append = false) {
  if (!isBound.value) return;
  if (append) {
    repoLoadingMore.value = true;
  } else {
    repoLoading.value = true;
  }
  try {
    const result = await listGitHubRepos(page);
    nextRepoPage.value = result.nextPage;
    repoItems.value = append
      ? dedupeRepos([...repoItems.value, ...result.items])
      : result.items;
  } finally {
    repoLoading.value = false;
    repoLoadingMore.value = false;
  }
}

async function ensureReposLoaded() {
  if (!isBound.value || repoLoading.value || repoItems.value.length > 0) return;
  await loadRepoPage(1);
}

async function maybeLoadMoreRepos() {
  if (!isBound.value || !nextRepoPage.value || repoLoadingMore.value) return;
  if (filteredRepos.value.length > 0 && queryTrimmed.value) return;
  await loadRepoPage(nextRepoPage.value, true);
}

async function init() {
  query.value = "";
  cloneError.value = null;
  cloneBusy.value = false;
  selectedRepo.value = null;
  repoDropdownOpen.value = false;
  repoItems.value = [];
  nextRepoPage.value = null;
  try {
    const [settings] = await Promise.all([
      getProjectSettings(),
      loadBindingStatus(),
    ]);
    if (settings.cloneParentDir && settings.cloneParentDir.trim()) {
      cloneParent.value = settings.cloneParentDir.trim();
    } else {
      cloneParent.value = await safeHomeDir();
    }
    if (isBound.value) {
      await ensureReposLoaded();
    }
  } catch {
    cloneParent.value = await safeHomeDir();
  }
  await nextTick();
  cloneInput.value?.focus();
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

function selectRepo(repo: GitHubRepoSummary) {
  selectedRepo.value = repo;
  query.value = repo.fullName;
  repoDropdownOpen.value = false;
}

function clearSelectedRepoIfNeeded() {
  if (selectedRepo.value && selectedRepo.value.fullName !== queryTrimmed.value) {
    selectedRepo.value = null;
  }
}

async function handleSearchFocus() {
  repoDropdownOpen.value = true;
  await ensureReposLoaded();
}

async function handleSearchInput() {
  clearSelectedRepoIfNeeded();
  repoDropdownOpen.value = true;
  await ensureReposLoaded();
  if (queryTrimmed.value) {
    await maybeLoadMoreRepos();
  }
}

async function confirmClone() {
  if (cloneBusy.value) return;
  cloneError.value = null;
  const input = queryTrimmed.value;
  const parent = cloneParent.value.trim();
  if (!input) {
    cloneError.value = isBound.value ? "请选择或输入 GitHub 仓库" : "请填写仓库地址";
    return;
  }
  if (!parent) {
    cloneError.value = "请选择 clone 目标父目录";
    return;
  }

  cloneBusy.value = true;
  try {
    const selected = selectedRepo.value?.fullName;
    const githubRepo = selected ?? directCloneRepo.value;
    const cloned = githubRepo
      ? await gitHubCloneRepo(githubRepo, parent)
      : await gitCloneRepo(input, parent);
    const project = await createProject({
      name: deriveProjectName(cloned) || "新项目",
      cwd: cloned,
    });
    emit("cloned", project);
    emit("close");
  } catch (err) {
    cloneError.value = String(err);
  } finally {
    cloneBusy.value = false;
  }
}

onMounted(() => {
  void init();
});
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        class="search-palette"
        role="dialog"
        aria-modal="true"
        aria-label="从 GitHub clone"
        @click.self="emit('close')"
      >
        <div class="search-palette__card dialog__card">
          <div class="dialog__header">
            <Github :size="14" aria-hidden="true" />
            <span>从 Git 仓库克隆</span>
          </div>
          <div class="dialog__body">
            <label v-if="!isBound">
              <span>仓库地址</span>
              <input
                ref="cloneInput"
                v-model="query"
                type="text"
                class="text-input"
                placeholder="owner/repo 或 https://github.com/owner/repo.git"
                @keydown.enter.prevent="confirmClone"
              />
            </label>

            <label v-else style="position: relative;">
              <span>GitHub 仓库</span>
              <div class="sb-search" style="height: 34px;">
                <Search :size="14" aria-hidden="true" class="sb-search__leading" />
                <input
                  ref="cloneInput"
                  v-model="query"
                  type="text"
                  class="sb-search__input"
                  placeholder="搜索仓库，或直接输入 owner/repo"
                  spellcheck="false"
                  @focus="handleSearchFocus"
                  @input="handleSearchInput"
                  @keydown.enter.prevent="confirmClone"
                />
              </div>

              <div v-if="repoDropdownOpen" class="sb-search-dd" role="listbox" style="position: relative; top: 6px;">
                <template v-if="filteredRepos.length">
                  <button
                    v-for="repo in filteredRepos"
                    :key="repo.id"
                    type="button"
                    class="sb-search-dd__item"
                    :class="{ 'is-active': selectedRepo?.id === repo.id }"
                    role="option"
                    :aria-selected="selectedRepo?.id === repo.id"
                    @click="selectRepo(repo)"
                  >
                    <span class="sb-search-dd__title">{{ repo.fullName }}</span>
                    <span class="sb-search-dd__scope">
                      <Lock v-if="repo.private" :size="11" aria-hidden="true" />
                      <template v-if="repo.private">私有</template>
                      <template v-else>公开</template>
                    </span>
                  </button>
                </template>
                <button
                  v-else-if="directCloneRepo"
                  type="button"
                  class="sb-search-dd__item"
                  role="option"
                  aria-selected="false"
                  @click="selectedRepo = null; repoDropdownOpen = false"
                >
                  <span class="sb-search-dd__title">直接克隆 {{ directCloneRepo }}</span>
                  <span class="sb-search-dd__scope">
                    <Sparkles :size="11" aria-hidden="true" />
                    手动输入
                  </span>
                </button>
                <p v-else-if="repoLoading" class="sb-search-dd__hint">正在加载仓库…</p>
                <p v-else class="sb-search-dd__empty">没有匹配仓库</p>
              </div>
            </label>

            <label>
              <span>目标父目录</span>
              <div class="dialog__field-row">
                <input
                  :value="cloneParent"
                  type="text"
                  class="text-input"
                  placeholder="选择克隆到哪个目录下"
                  readonly
                />
                <button type="button" class="ghost" :disabled="cloneBusy" @click="pickCloneParent">
                  <FolderOpen :size="12" aria-hidden="true" /> 选择
                </button>
              </div>
            </label>

            <p v-if="cloneTargetPreview" class="plugins-create__hint">
              将克隆到 <code>{{ cloneTargetPreview }}</code>
            </p>
            <p v-if="isBound && bindingStatus?.binding" class="plugins-create__hint">
              当前绑定账号：<code>{{ bindingStatus.binding.login }}</code>
            </p>
            <p v-if="cloneError" class="plugins-create__error">{{ cloneError }}</p>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ghost" :disabled="cloneBusy" @click="emit('close')">取消</button>
            <button type="button" class="primary" :disabled="cloneBusy" @click="confirmClone">
              {{ cloneBusy ? "克隆中…" : "克隆并添加" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
