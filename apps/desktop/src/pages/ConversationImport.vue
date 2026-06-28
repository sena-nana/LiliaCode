<script setup lang="ts">
import "../styles/chat.css";
import "../styles/pages/conversation-import.css";
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Check,
  ChevronDown,
  Clock3,
  Code2,
  Loader2,
  Search,
  TerminalSquare,
} from "@lucide/vue";
import type {
  AgentTimelineEvent,
  HistoryImportItem,
  HistoryImportProvider,
  HistoryImportRuntimeState,
} from "@lilia/contracts";
import { historyImportProviderDisplay, historyImportProviderUiLabels } from "@lilia/contracts";
import {
  attachHistoryImport,
  cleanHistoryImportBackgroundTerminals,
  listHistoryImportRuntimeStates,
  previewHistoryImport,
  searchHistoryImports,
} from "../services/chat";
import { ensureOrphansLoaded, ensureProjectTasksLoaded } from "../services/tasksStore";
import {
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../utils/perf";
import { createLazyLoadState } from "../utils/lazyLoadState";

const agentTimelineLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "import.timeline.load",
    async () => (await import("../components/chat/AgentTimeline.vue")).default,
  )
);
const chatScrollMapLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "import.scroll-map.load",
    async () => (await import("../components/chat/ChatScrollMap.vue")).default,
  )
);

const AgentTimeline = defineAsyncComponent({
  suspensible: false,
  loader: () => agentTimelineLoad.load(),
});

const ChatScrollMap = defineAsyncComponent({
  suspensible: false,
  loader: () => chatScrollMapLoad.load(),
});

const route = useRoute();
const router = useRouter();

const source = ref<HistoryImportProvider>("codex");
const query = ref("");
const includeArchived = ref(false);
const loading = ref(false);
const loadingMore = ref(false);
const importing = ref(false);
const error = ref("");
const importError = ref("");
const items = ref<HistoryImportItem[]>([]);
const nextCursor = ref<string | null>(null);
const runtimeStates = ref<HistoryImportRuntimeState[]>([]);
const cleaningThreadId = ref<string | null>(null);
const rowMessages = ref<Record<string, { kind: "ok" | "error"; text: string }>>({});
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
const previewScrollMapReady = ref(false);
let previewScrollMapIdleHandle: number | null = null;
let cancelPreviewScrollMapPaint: (() => void) | null = null;
let disposed = false;

const routeProjectId = computed(() => {
  const value = route.query.projectId;
  return Array.isArray(value) ? value[0] : value;
});

const isCodexSource = computed(() => source.value === "codex");
const sourceDisplay = computed(() => historyImportProviderDisplay(source.value));
const sourceLabels = computed(() => historyImportProviderUiLabels(source.value));

const importTargetLabel = computed(() =>
  routeProjectId.value ? "导入到当前项目" : "导入到收集箱",
);

const selectedItem = computed(() =>
  items.value.find((item) => item.id === selectedItemId.value) ?? null,
);

const runtimeByItemId = computed(() => {
  const map = new Map<string, HistoryImportRuntimeState>();
  for (const state of runtimeStates.value) map.set(state.itemId, state);
  return map;
});

const importRows = computed(() =>
  items.value.map((item) => {
    const runtime = isCodexSource.value
      ? runtimeByItemId.value.get(item.id) ?? item.runtime ?? null
      : null;
    return {
      item,
      runtime,
      canCleanHistoryImport: runtime?.running === true,
    };
  }),
);

const sourceListLabel = computed(() => sourceLabels.value.list);
const sourcePreviewLabel = computed(() => sourceLabels.value.preview);
const searchPlaceholder = computed(() => sourceLabels.value.searchPlaceholder);
const loadingLabel = computed(() => sourceLabels.value.loading);
const emptyListLabel = computed(() => sourceLabels.value.emptyList);
const emptyPreviewLabel = computed(() => sourceLabels.value.emptyPreview);
const choosePreviewLabel = computed(() => sourceLabels.value.choosePreview);
const showArchivedToggle = computed(() => sourceDisplay.value.supportsArchived);

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

function localItemFromRuntime(state: HistoryImportRuntimeState): HistoryImportItem {
  return {
    id: state.itemId,
    provider: "codex",
    title: state.taskTitle.trim() || state.itemId,
    status: state.running ? "running" : state.queued ? "queued" : null,
    model: null,
    sourceKind: "lilia",
    createdAt: null,
    updatedAt: null,
    archived: false,
    preview: null,
    runtime: state,
  };
}

function runtimeMatchesSearch(state: HistoryImportRuntimeState, searchTerm: string): boolean {
  const term = searchTerm.trim().toLowerCase();
  if (!term) return true;
  return [state.taskTitle, state.itemId]
    .some((value) => value.toLowerCase().includes(term));
}

function localItemsFromRuntimeStates(
  states: HistoryImportRuntimeState[],
  searchTerm: string,
): HistoryImportItem[] {
  return states
    .filter((state) => runtimeMatchesSearch(state, searchTerm))
    .map(localItemFromRuntime);
}

function mergeCodexItem(
  existing: HistoryImportItem,
  incoming: HistoryImportItem,
): HistoryImportItem {
  return {
    id: existing.id,
    provider: "codex",
    title: incoming.title.trim() ? incoming.title : existing.title,
    status: incoming.status ?? existing.status,
    model: incoming.model ?? existing.model,
    sourceKind: incoming.sourceKind ?? existing.sourceKind,
    createdAt: incoming.createdAt ?? existing.createdAt,
    updatedAt: incoming.updatedAt ?? existing.updatedAt,
    archived: incoming.archived || existing.archived,
    preview: incoming.preview ?? existing.preview,
    runtime: existing.runtime ?? incoming.runtime ?? null,
  };
}

function mergeCodexItems(
  base: HistoryImportItem[],
  incoming: HistoryImportItem[],
): HistoryImportItem[] {
  const indexById = new Map<string, number>();
  const out = base.map((item, index) => {
    indexById.set(item.id, index);
    return item;
  });
  for (const item of incoming) {
    const index = indexById.get(item.id);
    if (index === undefined) {
      indexById.set(item.id, out.length);
      out.push(item);
    } else {
      out[index] = mergeCodexItem(out[index], item);
    }
  }
  return out;
}

function selectFirstItemIfNeeded() {
  if (!selectedItemId.value && items.value[0]) {
    void selectHistoryImport(items.value[0]);
  }
}

async function loadRuntimeStates(): Promise<HistoryImportRuntimeState[]> {
  if (!isCodexSource.value) return [];
  try {
    const states = await listHistoryImportRuntimeStates();
    if (!disposed) runtimeStates.value = states;
    return states;
  } catch (err) {
    console.error("[conversation-import] Codex runtime states load failed", err);
    if (!disposed) runtimeStates.value = [];
    return [];
  }
}

async function loadHistoryImports(cursor: string | null = null) {
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
      provider: currentSource,
    };
    if (currentSource === "codex" && !append) {
      const runtimePromise = loadRuntimeStates();
      const searchPromise = searchHistoryImports(input);
      const states = await runtimePromise;
      if (seq !== searchSeq) return;
      items.value = localItemsFromRuntimeStates(states, input.searchTerm ?? "");
      nextCursor.value = null;
      loading.value = false;
      selectFirstItemIfNeeded();

      let result;
      try {
        result = await searchPromise;
      } catch (err) {
        if (seq === searchSeq && items.value.length === 0) error.value = String(err);
        else console.error("[conversation-import] Codex history search failed", err);
        return;
      }
      if (seq !== searchSeq) return;
      items.value = mergeCodexItems(items.value, result.items);
      nextCursor.value = result.nextCursor;
    } else {
      const result = await searchHistoryImports(input);
      if (seq !== searchSeq) return;
      items.value = append && currentSource === "codex"
        ? mergeCodexItems(items.value, result.items)
        : append
          ? [...items.value, ...result.items]
          : result.items;
      nextCursor.value = result.nextCursor;
    }
    selectFirstItemIfNeeded();
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
    searchTimer = null;
    if (disposed) return;
    resetPreviewState();
    rowMessages.value = {};
    importError.value = "";
    void loadHistoryImports();
  }, 240);
}

function cancelSearchSchedule() {
  if (searchTimer) {
    clearTimeout(searchTimer);
    searchTimer = null;
  }
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

async function selectHistoryImport(item: HistoryImportItem) {
  if (disposed) return;
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
    const result = await previewHistoryImport({
      provider: source.value,
      itemId: item.id,
      detail: "lite",
    });
    if (disposed || seq !== previewSeq) return;
    previewEventCount.value = result.hasFullPreview ? null : result.eventCount;
    if (result.hasFullPreview) void loadFullPreview(item.id);
  } catch (err) {
    if (!disposed && seq === previewSeq) previewError.value = String(err);
  } finally {
    if (!disposed && seq === previewSeq) previewLoading.value = false;
  }
}

async function loadFullPreview(itemId: string = selectedItem.value?.id ?? "") {
  if (disposed || !itemId || fullPreviewLoading.value || fullPreviewEvents.value.length > 0) return;
  fullPreviewLoading.value = true;
  fullPreviewError.value = "";
  const seq = ++fullPreviewSeq;
  try {
    const result = await previewHistoryImport({
      provider: source.value,
      itemId,
      detail: "full",
    });
    if (!disposed && seq === fullPreviewSeq) {
      fullPreviewEvents.value = result.events;
      previewEventCount.value = result.eventCount;
    }
  } catch (err) {
    if (!disposed && seq === fullPreviewSeq) fullPreviewError.value = String(err);
  } finally {
    if (!disposed && seq === fullPreviewSeq) fullPreviewLoading.value = false;
  }
}

function showPreviewScrollbar() {
  previewScrollMap.value?.show();
}

function cancelPreviewScrollMapSchedule() {
  cancelPreviewScrollMapPaint?.();
  cancelPreviewScrollMapPaint = null;
  if (previewScrollMapIdleHandle !== null) {
    cancelIdleRun(previewScrollMapIdleHandle);
    previewScrollMapIdleHandle = null;
  }
}

function schedulePreviewScrollMap() {
  cancelPreviewScrollMapSchedule();
  if (!fullPreviewEvents.value.length) {
    previewScrollMapReady.value = false;
    return;
  }
  cancelPreviewScrollMapPaint = scheduleAfterPaint(() => {
    cancelPreviewScrollMapPaint = null;
    if (disposed || !fullPreviewEvents.value.length) return;
    previewScrollMapIdleHandle = runWhenIdle(() => {
      previewScrollMapIdleHandle = null;
      if (disposed || !fullPreviewEvents.value.length) return;
      previewScrollMapReady.value = true;
    });
  });
}

async function importSelectedItem() {
  const item = selectedItem.value;
  if (disposed || !item || importing.value) return;
  importing.value = true;
  importError.value = "";
  try {
    const result = await attachHistoryImport({
      provider: source.value,
      mode: "new",
      itemId: item.id,
      taskId: null,
      projectId: routeProjectId.value ?? null,
      item,
    });
    if (disposed) return;
    if (result.projectId) {
      await ensureProjectTasksLoaded(result.projectId, true);
      if (disposed) return;
      await router.push(`/projects/${result.projectId}/tasks/${result.taskId}`);
    } else {
      await ensureOrphansLoaded(true);
      if (disposed) return;
      await router.push(`/chats/${result.taskId}`);
    }
  } catch (err) {
    if (!disposed) importError.value = String(err);
  } finally {
    if (!disposed) importing.value = false;
  }
}

function setSource(next: HistoryImportProvider) {
  if (source.value === next) return;
  source.value = next;
  if (next === "claude") includeArchived.value = false;
  resetPreviewState();
  items.value = [];
  nextCursor.value = null;
  if (next === "claude") runtimeStates.value = [];
  rowMessages.value = {};
  importError.value = "";
  void loadHistoryImports();
}

async function cleanHistoryImport(item: HistoryImportItem) {
  if (disposed || !isCodexSource.value || cleaningThreadId.value) return;
  cleaningThreadId.value = item.id;
  rowMessages.value = {
    ...rowMessages.value,
    [item.id]: { kind: "ok", text: "正在清理后台终端..." },
  };
  try {
    await cleanHistoryImportBackgroundTerminals(item.id);
    if (disposed) return;
    rowMessages.value = {
      ...rowMessages.value,
      [item.id]: { kind: "ok", text: "后台终端已清理" },
    };
    await loadRuntimeStates();
  } catch (err) {
    if (!disposed) rowMessages.value = {
      ...rowMessages.value,
      [item.id]: { kind: "error", text: `清理失败：${String(err)}` },
    };
  } finally {
    if (!disposed) cleaningThreadId.value = null;
  }
}

watch(() => query.value, scheduleSearch);
watch(() => includeArchived.value, () => {
  if (source.value === "codex") scheduleSearch();
});

onMounted(() => {
  disposed = false;
  void loadHistoryImports();
});

onBeforeUnmount(() => {
  disposed = true;
  searchSeq += 1;
  previewSeq += 1;
  fullPreviewSeq += 1;
  cancelSearchSchedule();
  cancelPreviewScrollMapSchedule();
});

watch(
  () => fullPreviewEvents.value.length,
  () => {
    schedulePreviewScrollMap();
  },
);
</script>

<template>
  <section class="conversation-import-page" data-agent-id="conversation-import.page">
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
                data-agent-id="conversation-import.source.codex"
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
                data-agent-id="conversation-import.source.claude"
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
                data-agent-id="conversation-import.search"
              />
            </label>
            <label v-if="showArchivedToggle" class="conversation-import__toggle ui-switch">
              <input v-model="includeArchived" type="checkbox" data-agent-id="conversation-import.include-archived" />
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
              <div
                v-for="row in importRows"
                :key="row.item.id"
                class="conversation-import__row ui-list-item"
                :class="{ 'is-active': selectedItemId === row.item.id }"
              >
                <button
                  type="button"
                  class="conversation-import__row-select"
                  :data-agent-id="`conversation-import.item.${row.item.id}.select`"
                  :title="row.item.title"
                  @click="selectHistoryImport(row.item)"
                >
                  <span class="conversation-import__row-head">
                    <span class="conversation-import__row-title">{{ row.item.title }}</span>
                    <span
                      v-if="row.runtime || (row.item.updatedAt ?? row.item.createdAt)"
                      class="conversation-import__row-time"
                    >
                      {{ row.runtime ? "LiliaCode" : formatTime(row.item.updatedAt ?? row.item.createdAt) }}
                    </span>
                  </span>
                  <span v-if="source === 'codex'" class="conversation-import__row-badges">
                    <span v-if="row.runtime?.running" class="ui-badge conversation-import__status--running">
                      运行中
                    </span>
                    <span v-else-if="row.runtime?.queued" class="ui-badge conversation-import__status--queued">
                      排队中
                    </span>
                    <span v-if="row.item.archived" class="ui-badge ui-badge--muted">已归档</span>
                  </span>
                  <span
                    v-if="rowMessages[row.item.id]"
                    class="conversation-import__row-message"
                    :class="`is-${rowMessages[row.item.id].kind}`"
                  >
                    <Check
                      v-if="rowMessages[row.item.id].kind === 'ok'"
                      :size="13"
                      aria-hidden="true"
                    />
                    <span>{{ rowMessages[row.item.id].text }}</span>
                  </span>
                </button>
                <div v-if="row.canCleanHistoryImport" class="conversation-import__row-actions">
                  <button
                    type="button"
                    class="conversation-import__clean-button ui-button ui-button--ghost"
                    :disabled="cleaningThreadId !== null"
                    title="清理后台终端"
                    aria-label="清理后台终端"
                    :data-agent-id="`conversation-import.item.${row.item.id}.clean`"
                    @click="cleanHistoryImport(row.item)"
                  >
                    <Loader2
                      v-if="cleaningThreadId === row.item.id"
                      :size="14"
                      class="is-spinning"
                      aria-hidden="true"
                    />
                    <TerminalSquare v-else :size="14" aria-hidden="true" />
                  </button>
                </div>
              </div>
            </template>
            <button
              v-if="nextCursor"
              type="button"
              class="conversation-import__more ui-button ui-button--ghost"
              data-agent-id="conversation-import.load-more"
              :disabled="loadingMore"
              @click="loadHistoryImports(nextCursor)"
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
                data-agent-id="conversation-import.import"
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
                data-no-global-scrollbar-overlay
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
                v-if="previewScrollMapReady"
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

