<script setup lang="ts">
import "../styles/chat.css";
import "../styles/pages/conversation-import.css";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Check,
  ChevronDown,
  Clock3,
  Code2,
  Loader2,
  Search,
} from "lucide-vue-next";
import type { AgentTimelineEvent, ClaudeSessionSummary, CodexThreadSummary } from "@lilia/contracts";
import {
  attachClaudeSession,
  attachCodexThread,
  previewClaudeSession,
  previewCodexThread,
  searchClaudeSessions,
  searchCodexThreads,
} from "../services/chat";
import { ensureOrphansLoaded, ensureProjectTasksLoaded } from "../services/tasksStore";
import AgentTimeline from "../components/chat/AgentTimeline.vue";
import ChatScrollMap from "../components/chat/ChatScrollMap.vue";

const route = useRoute();
const router = useRouter();
type ImportSource = "codex" | "claude";
type ImportItem = CodexThreadSummary | ClaudeSessionSummary;

const source = ref<ImportSource>("codex");
const query = ref("");
const includeArchived = ref(false);
const loading = ref(false);
const loadingMore = ref(false);
const importing = ref(false);
const error = ref("");
const importError = ref("");
const items = ref<ImportItem[]>([]);
const nextCursor = ref<string | null>(null);
const selectedItemId = ref<string | null>(null);
const previewLoading = ref(false);
const previewError = ref("");
const fullPreviewLoading = ref(false);
const fullPreviewError = ref("");
let searchSeq = 0;
let searchTimer: ReturnType<typeof setTimeout> | null = null;
let previewSeq = 0;
let fullPreviewSeq = 0;

const previewEventCount = ref<number | null>(null);
const fullPreviewEvents = ref<AgentTimelineEvent[]>([]);
const previewFrame = ref<HTMLElement | null>(null);
const previewScroller = ref<HTMLElement | null>(null);
const previewScrollMap = ref<{ show: () => void } | null>(null);

const routeProjectId = computed(() => {
  const value = route.query.projectId;
  return Array.isArray(value) ? value[0] : value;
});

const importTargetLabel = computed(() =>
  routeProjectId.value ? "导入到当前项目" : "导入到收集箱",
);

const selectedItem = computed(() =>
  items.value.find((item) => item.id === selectedItemId.value) ?? null,
);

const sourceLabel = computed(() => source.value === "claude" ? "Claude session" : "Codex thread");
const sourceListLabel = computed(() => `${sourceLabel.value} 列表`);
const sourcePreviewLabel = computed(() => `${sourceLabel.value} 预览`);
const searchPlaceholder = computed(() => `搜索 ${sourceLabel.value}`);
const loadingLabel = computed(() => `正在读取 ${source.value === "claude" ? "Claude" : "Codex"} 历史`);
const emptyListLabel = computed(() => `没有找到 ${sourceLabel.value}`);
const emptyPreviewLabel = computed(() => `这个 ${sourceLabel.value} 暂无可预览事件。`);
const choosePreviewLabel = computed(() => `选择一个 ${sourceLabel.value} 后查看摘要并导入。`);
const showArchivedToggle = computed(() => source.value === "codex");

const selectedItemMeta = computed(() => {
  const item = selectedItem.value;
  if (!item) return "";
  const parts = [formatTime(item.updatedAt ?? item.createdAt)];
  if (previewEventCount.value) parts.push(`${previewEventCount.value} 条事件`);
  if (item.status) parts.push(item.status);
  return parts.join(" · ");
});

function formatTime(value: number | null): string {
  if (!value) return "未知时间";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

async function loadThreads(cursor: string | null = null) {
  const seq = ++searchSeq;
  const append = !!cursor;
  const currentSource = source.value;
  if (append) loadingMore.value = true;
  else loading.value = true;
  error.value = "";
  try {
    const input = {
      searchTerm: query.value.trim() || null,
      cursor,
      limit: 20,
      archived: currentSource === "codex" ? includeArchived.value : null,
    };
    if (currentSource === "claude") {
      const result = await searchClaudeSessions(input);
      if (seq !== searchSeq) return;
      items.value = append ? [...items.value, ...result.sessions] : result.sessions;
      nextCursor.value = result.nextCursor;
    } else {
      const result = await searchCodexThreads(input);
      if (seq !== searchSeq) return;
      items.value = append ? [...items.value, ...result.threads] : result.threads;
      nextCursor.value = result.nextCursor;
    }
    if (!selectedItemId.value && items.value[0]) {
      void selectItem(items.value[0]);
    }
  } catch (err) {
    if (seq === searchSeq) error.value = String(err);
  } finally {
    if (seq === searchSeq) {
      loading.value = false;
      loadingMore.value = false;
    }
  }
}

function scheduleSearch() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    resetPreviewState();
    importError.value = "";
    void loadThreads();
  }, 240);
}

function resetPreviewState() {
  previewSeq += 1;
  fullPreviewSeq += 1;
  selectedItemId.value = null;
  previewEventCount.value = null;
  fullPreviewEvents.value = [];
  previewError.value = "";
  previewLoading.value = false;
  fullPreviewError.value = "";
  fullPreviewLoading.value = false;
}

async function selectItem(item: ImportItem) {
  selectedItemId.value = item.id;
  previewEventCount.value = null;
  fullPreviewEvents.value = [];
  previewError.value = "";
  fullPreviewError.value = "";
  fullPreviewLoading.value = false;
  importError.value = "";
  previewLoading.value = true;
  const seq = ++previewSeq;
  fullPreviewSeq += 1;
  try {
    const result = source.value === "claude"
      ? await previewClaudeSession({ sessionId: item.id, detail: "lite" })
      : await previewCodexThread({ threadId: item.id, detail: "lite" });
    if (seq !== previewSeq) return;
    previewEventCount.value = result.hasFullPreview ? null : result.eventCount;
    if (result.hasFullPreview) void loadFullPreview(item.id);
  } catch (err) {
    if (seq === previewSeq) previewError.value = String(err);
  } finally {
    if (seq === previewSeq) previewLoading.value = false;
  }
}

async function loadFullPreview(itemId: string = selectedItem.value?.id ?? "") {
  if (!itemId || fullPreviewLoading.value || fullPreviewEvents.value.length > 0) return;
  fullPreviewLoading.value = true;
  fullPreviewError.value = "";
  const seq = ++fullPreviewSeq;
  try {
    const result = source.value === "claude"
      ? await previewClaudeSession({ sessionId: itemId, detail: "full" })
      : await previewCodexThread({ threadId: itemId, detail: "full" });
    if (seq === fullPreviewSeq) {
      fullPreviewEvents.value = result.events;
      previewEventCount.value = result.eventCount;
    }
  } catch (err) {
    if (seq === fullPreviewSeq) fullPreviewError.value = String(err);
  } finally {
    if (seq === fullPreviewSeq) fullPreviewLoading.value = false;
  }
}

function showPreviewScrollbar() {
  previewScrollMap.value?.show();
}

async function importSelectedItem() {
  const item = selectedItem.value;
  if (!item || importing.value) return;
  importing.value = true;
  importError.value = "";
  try {
    const result = source.value === "claude"
      ? await attachClaudeSession({
        mode: "new",
        sessionId: item.id,
        taskId: null,
        projectId: routeProjectId.value ?? null,
        session: item as ClaudeSessionSummary,
      })
      : await attachCodexThread({
        mode: "new",
        threadId: item.id,
        taskId: null,
        projectId: routeProjectId.value ?? null,
        thread: item as CodexThreadSummary,
      });
    if (result.projectId) {
      await ensureProjectTasksLoaded(result.projectId, true);
      await router.push(`/projects/${result.projectId}/tasks/${result.taskId}`);
    } else {
      await ensureOrphansLoaded(true);
      await router.push(`/chats/${result.taskId}`);
    }
  } catch (err) {
    importError.value = String(err);
  } finally {
    importing.value = false;
  }
}

function setSource(next: ImportSource) {
  if (source.value === next) return;
  source.value = next;
  if (next === "claude") includeArchived.value = false;
  resetPreviewState();
  items.value = [];
  nextCursor.value = null;
  importError.value = "";
  void loadThreads();
}

watch(() => query.value, scheduleSearch);
watch(() => includeArchived.value, () => {
  if (source.value === "codex") scheduleSearch();
});

onMounted(() => {
  void loadThreads();
});
</script>

<template>
  <section class="conversation-import-page">
    <div class="conversation-import">
      <div class="conversation-import__content">
        <aside class="conversation-import__sidebar" aria-label="导入来源和对话列表">
          <div class="conversation-import__source-bar">
            <div class="conversation-import__tabs ui-tabs ui-tabs--pill" role="tablist" aria-label="导入来源">
              <button
                type="button"
                class="ui-tabs__tab"
                :class="{ 'is-active': source === 'codex' }"
                role="tab"
                :aria-selected="source === 'codex'"
                @click="setSource('codex')"
              >
                <Code2 :size="13" aria-hidden="true" />
                <span>Codex</span>
              </button>
              <button
                type="button"
                class="ui-tabs__tab"
                :class="{ 'is-active': source === 'claude' }"
                role="tab"
                :aria-selected="source === 'claude'"
                @click="setSource('claude')"
              >
                <Clock3 :size="13" aria-hidden="true" />
                <span>Claude</span>
              </button>
            </div>
          </div>

          <div class="conversation-import__search">
            <label class="conversation-import__searchbox">
              <Search :size="14" aria-hidden="true" />
              <input
                v-model="query"
                type="search"
                :placeholder="searchPlaceholder"
                :aria-label="searchPlaceholder"
              />
            </label>
            <label v-if="showArchivedToggle" class="conversation-import__toggle ui-switch">
              <input v-model="includeArchived" type="checkbox" />
              <span>包含归档</span>
            </label>
          </div>

          <section class="conversation-import__list ui-list" :aria-label="sourceListLabel">
            <div v-if="error" class="conversation-import__notice is-error">{{ error }}</div>
            <div v-else-if="loading" class="conversation-import__notice">
              <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
              <span>{{ loadingLabel }}</span>
            </div>
            <div v-else-if="items.length === 0" class="conversation-import__notice">
              {{ emptyListLabel }}
            </div>
            <template v-else>
              <button
                v-for="item in items"
                :key="item.id"
                type="button"
                class="conversation-import__row ui-list-item"
                :class="{ 'is-active': selectedItemId === item.id }"
                :title="item.title"
                @click="selectItem(item)"
              >
                <span class="conversation-import__row-title">{{ item.title }}</span>
                <span class="conversation-import__row-time">
                  {{ formatTime(item.updatedAt ?? item.createdAt) }}
                </span>
              </button>
            </template>
            <button
              v-if="nextCursor"
              type="button"
              class="conversation-import__more ui-button ui-button--ghost"
              :disabled="loadingMore"
              @click="loadThreads(nextCursor)"
            >
              <Loader2 v-if="loadingMore" :size="14" class="is-spinning" aria-hidden="true" />
              <ChevronDown v-else :size="14" aria-hidden="true" />
              <span>加载更多</span>
            </button>
          </section>
        </aside>

        <section class="conversation-import__preview" :aria-label="sourcePreviewLabel">
          <template v-if="selectedItem">
            <div class="conversation-import__preview-head">
              <div class="conversation-import__preview-heading">
                <div class="conversation-import__preview-title">{{ selectedItem.title }}</div>
                <div class="conversation-import__preview-meta">
                  {{ selectedItemMeta }}
                </div>
              </div>
              <button
                type="button"
                class="conversation-import__import-button ui-button ui-button--primary"
                :disabled="!selectedItem || importing"
                @click="importSelectedItem"
              >
                <Loader2 v-if="importing" :size="14" class="is-spinning" aria-hidden="true" />
                <Check v-else :size="14" aria-hidden="true" />
                <span>{{ importing ? "导入中…" : importTargetLabel }}</span>
              </button>
            </div>

            <div ref="previewFrame" class="conversation-import__timeline-frame">
              <div
                ref="previewScroller"
                class="conversation-import__timeline-scroller"
                @scroll="showPreviewScrollbar"
              >
                <div
                  v-if="previewLoading || fullPreviewLoading"
                  class="conversation-import__notice conversation-import__timeline-loading"
                >
                  <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
                  <span>读取中</span>
                </div>
                <div v-else-if="previewError" class="conversation-import__notice is-error">
                  {{ previewError }}
                </div>
                <div v-else-if="fullPreviewError" class="conversation-import__notice is-error">
                  {{ fullPreviewError }}
                </div>
                <AgentTimeline
                  v-else-if="fullPreviewEvents.length"
                  :events="fullPreviewEvents"
                />
                <div v-else class="conversation-import__empty-preview">
                  {{ emptyPreviewLabel }}
                </div>
              </div>
              <ChatScrollMap
                ref="previewScrollMap"
                :events="fullPreviewEvents"
                :hover-target="previewFrame"
                :scroller="previewScroller"
              />
            </div>
          </template>
          <div
            v-else-if="loading"
            class="conversation-import__notice conversation-import__timeline-loading"
          >
            <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
            <span>读取中</span>
          </div>
          <div v-else class="conversation-import__empty-preview">
            {{ choosePreviewLabel }}
          </div>
        </section>
      </div>

      <div v-if="importError" class="conversation-import__import-error">
        {{ importError }}
      </div>
    </div>
  </section>
</template>
