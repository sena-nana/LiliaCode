<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { FolderOpen, Github, Lock, Sparkles } from "lucide-vue-next";
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
  isGitHubBindingExpiredError,
  listGitHubRepos,
  pickFolder,
  preloadGitHubRepos,
  readCachedGitHubRepos,
} from "../../services/projects";
import SearchDropdown from "../SearchDropdown.vue";

const emit = defineEmits<{
  close: [];
  cloned: [project: Project];
  error: [msg: string];
}>();

const router = useRouter();
const cloneInput = ref<{ focus: (options?: FocusOptions & { open?: boolean }) => void } | null>(null);
const cloneBusy = ref(false);
const cloneError = ref<string | null>(null);
const cloneParent = ref("");
const query = ref("");
const bindingStatus = ref<GitHubBindingStatus | null>(null);
const repoItems = ref<GitHubRepoSummary[]>([]);
const repoDropdownOpen = ref(false);
const repoLoading = ref(false);
const repoLoadingMore = ref(false);
const repoLoadError = ref<string | null>(null);
const nextRepoPage = ref<number | null>(null);
const selectedRepo = ref<GitHubRepoSummary | null>(null);

const isBound = computed(() => bindingStatus.value?.state === "bound");
const queryTrimmed = computed(() => query.value.trim());
const githubBindingExpired = computed(() =>
  cloneError.value?.includes("GitHub 绑定已失效") === true
);

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

function applyRepoPage(
  result: { items: GitHubRepoSummary[]; nextPage: number | null },
  append = false,
) {
  repoLoadError.value = null;
  nextRepoPage.value = result.nextPage;
  repoItems.value = append
    ? dedupeRepos([...repoItems.value, ...result.items])
    : result.items;
}

function showRepoLoadError(err: unknown) {
  const expired = isGitHubBindingExpiredError(err);
  repoLoadError.value = expired
    ? "GitHub 绑定已失效，请重新绑定后再加载账号仓库。"
    : `仓库列表加载失败：${String(err)}`;
  if (expired) {
    cloneError.value = "GitHub 绑定已失效，请重新绑定。";
  }
  repoItems.value = [];
  nextRepoPage.value = null;
  selectedRepo.value = null;
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

async function loadRepoPage(
  page = 1,
  append = false,
  options: { force?: boolean; showLoading?: boolean } = {},
) {
  if (!isBound.value) return;
  const showLoading = options.showLoading ?? !append;
  if (append) {
    repoLoadingMore.value = true;
  } else if (showLoading) {
    repoLoading.value = true;
  }
  try {
    const result = page === 1
      ? await preloadGitHubRepos({ force: options.force })
      : await listGitHubRepos(page);
    applyRepoPage(result, append);
  } catch (err) {
    showRepoLoadError(err);
  } finally {
    if (showLoading) {
      repoLoading.value = false;
    }
    repoLoadingMore.value = false;
  }
}

function startReposLoad() {
  if (!isBound.value || repoLoading.value || repoItems.value.length > 0) return;
  void loadRepoPage(1).catch(() => undefined);
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
  repoLoadError.value = null;
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
      const cached = readCachedGitHubRepos();
      if (cached) {
        applyRepoPage(cached);
      }
      void loadRepoPage(1, false, {
        force: Boolean(cached),
        showLoading: !cached,
      }).catch(() => undefined);
    }
  } catch {
    cloneParent.value = await safeHomeDir();
  }
  await nextTick();
  cloneInput.value?.focus({ open: false });
}

async function openGitHubBindingSettings() {
  emit("close");
  await router.push({ path: "/settings", query: { tab: "project" } });
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

function openRepoDropdown() {
  repoDropdownOpen.value = true;
  startReposLoad();
}

function handleSearchInput() {
  clearSelectedRepoIfNeeded();
  repoDropdownOpen.value = true;
  startReposLoad();
  if (queryTrimmed.value) {
    void maybeLoadMoreRepos();
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
    if (isGitHubBindingExpiredError(err)) {
      showRepoLoadError(err);
    } else {
      cloneError.value = String(err);
    }
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
        <div class="search-palette__card dialog__card dialog__card--repo-dropdown">
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
                class="ui-input"
                placeholder="owner/repo 或 https://github.com/owner/repo.git"
                @keydown.enter.prevent="confirmClone"
              />
            </label>

            <div v-else class="dialog__field">
              <span>GitHub 仓库</span>
              <SearchDropdown
                ref="cloneInput"
                v-model="query"
                v-model:open="repoDropdownOpen"
                placeholder="搜索仓库，或直接输入 owner/repo"
                close-on-outside
                close-on-escape
                :spellcheck="false"
                @open-request="openRepoDropdown"
                @input="handleSearchInput"
                @keydown.enter.prevent="confirmClone"
              >
                <template #default="{ highlightQuerySegments }">
                  <template v-if="filteredRepos.length">
                    <button
                      v-for="repo in filteredRepos"
                      :key="repo.id"
                      type="button"
                      class="search-dropdown__item"
                      :class="{ 'is-active': selectedRepo?.id === repo.id }"
                      role="option"
                      :aria-selected="selectedRepo?.id === repo.id"
                      @click="selectRepo(repo)"
                    >
                      <span class="search-dropdown__title">
                        <template
                          v-for="(segment, segmentIndex) in highlightQuerySegments(repo.fullName)"
                          :key="segmentIndex"
                        >
                          <mark v-if="segment.mark">{{ segment.text }}</mark>
                          <template v-else>{{ segment.text }}</template>
                        </template>
                      </span>
                      <span class="search-dropdown__scope">
                        <Lock v-if="repo.private" :size="11" aria-hidden="true" />
                        <template v-if="repo.private">私有</template>
                        <template v-else>公开</template>
                      </span>
                    </button>
                  </template>
                  <button
                    v-else-if="directCloneRepo"
                    type="button"
                    class="search-dropdown__item"
                    role="option"
                    aria-selected="false"
                    @click="selectedRepo = null; repoDropdownOpen = false"
                  >
                    <span class="search-dropdown__title">直接克隆 {{ directCloneRepo }}</span>
                    <span class="search-dropdown__scope">
                      <Sparkles :size="11" aria-hidden="true" />
                      手动输入
                    </span>
                  </button>
                  <p v-else-if="repoLoadError" class="search-dropdown__empty">{{ repoLoadError }}</p>
                  <p v-else-if="repoLoading" class="search-dropdown__hint">正在加载仓库…</p>
                  <p v-else class="search-dropdown__empty">没有匹配仓库</p>
                </template>
              </SearchDropdown>
            </div>

            <label>
              <span>目标父目录</span>
              <div class="dialog__field-row">
                <input
                  :value="cloneParent"
                  type="text"
                  class="ui-input"
                  placeholder="选择克隆到哪个目录下"
                  readonly
                />
                <button type="button" class="ui-button ui-button--ghost" :disabled="cloneBusy" @click="pickCloneParent">
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
            <div v-if="cloneError" class="plugins-create__error dialog__field-row">
              <span>{{ cloneError }}</span>
              <button
                v-if="githubBindingExpired"
                type="button"
                class="ui-button ui-button--ghost"
                @click="openGitHubBindingSettings"
              >
                重新绑定 GitHub
              </button>
            </div>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ui-button ui-button--ghost" :disabled="cloneBusy" @click="emit('close')">取消</button>
            <button type="button" class="ui-button ui-button--primary" :disabled="cloneBusy" @click="confirmClone">
              {{ cloneBusy ? "克隆中…" : "克隆并添加" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
