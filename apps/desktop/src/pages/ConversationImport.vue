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
import type { AgentTimelineEvent, CodexThreadSummary } from "@lilia/contracts";
import {
  attachCodexThread,
  previewCodexThread,
  searchCodexThreads,
} from "../services/chat";
import { ensureOrphansLoaded, ensureProjectTasksLoaded } from "../services/tasksStore";
import AgentTimeline from "../components/chat/AgentTimeline.vue";
import ChatScrollMap from "../components/chat/ChatScrollMap.vue";

const route = useRoute();
const router = useRouter();
const query = ref("");
const includeArchived = ref(false);
const loading = ref(false);
const loadingMore = ref(false);
const importing = ref(false);
const error = ref("");
const importError = ref("");
const threads = ref<CodexThreadSummary[]>([]);
const nextCursor = ref<string | null>(null);
const selectedThreadId = ref<string | null>(null);
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

const selectedThread = computed(() =>
  threads.value.find((thread) => thread.id === selectedThreadId.value) ?? null,
);

const selectedThreadMeta = computed(() => {
  const thread = selectedThread.value;
  if (!thread) return "";
  const parts = [formatTime(thread.updatedAt ?? thread.createdAt)];
  if (previewEventCount.value) parts.push(`${previewEventCount.value} 条事件`);
  if (thread.status) parts.push(thread.status);
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
  if (append) loadingMore.value = true;
  else loading.value = true;
  error.value = "";
  try {
    const result = await searchCodexThreads({
      searchTerm: query.value.trim() || null,
      cursor,
      limit: 20,
      archived: includeArchived.value,
    });
    if (seq !== searchSeq) return;
    threads.value = append ? [...threads.value, ...result.threads] : result.threads;
    nextCursor.value = result.nextCursor;
    if (!selectedThreadId.value && threads.value[0]) {
      void selectThread(threads.value[0]);
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
    selectedThreadId.value = null;
    previewEventCount.value = null;
    fullPreviewEvents.value = [];
    importError.value = "";
    void loadThreads();
  }, 240);
}

async function selectThread(thread: CodexThreadSummary) {
  selectedThreadId.value = thread.id;
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
    const result = await previewCodexThread({ threadId: thread.id, detail: "lite" });
    if (seq !== previewSeq) return;
    previewEventCount.value = result.eventCount;
    if (result.hasFullPreview) void loadFullPreview(thread.id);
  } catch (err) {
    if (seq === previewSeq) previewError.value = String(err);
  } finally {
    if (seq === previewSeq) previewLoading.value = false;
  }
}

async function loadFullPreview(threadId: string = selectedThread.value?.id ?? "") {
  if (!threadId || fullPreviewLoading.value || fullPreviewEvents.value.length > 0) return;
  fullPreviewLoading.value = true;
  fullPreviewError.value = "";
  const seq = ++fullPreviewSeq;
  try {
    const result = await previewCodexThread({ threadId, detail: "full" });
    if (seq === fullPreviewSeq) fullPreviewEvents.value = result.events;
  } catch (err) {
    if (seq === fullPreviewSeq) fullPreviewError.value = String(err);
  } finally {
    if (seq === fullPreviewSeq) fullPreviewLoading.value = false;
  }
}

function showPreviewScrollbar() {
  previewScrollMap.value?.show();
}

async function importSelectedThread() {
  const thread = selectedThread.value;
  if (!thread || importing.value) return;
  importing.value = true;
  importError.value = "";
  try {
    const result = await attachCodexThread({
      mode: "new",
      threadId: thread.id,
      taskId: null,
      projectId: routeProjectId.value ?? null,
      thread,
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

watch(() => [query.value, includeArchived.value] as const, scheduleSearch);

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
                class="ui-tabs__tab is-active"
                role="tab"
                aria-selected="true"
              >
                <Code2 :size="13" aria-hidden="true" />
                <span>Codex</span>
              </button>
              <button
                type="button"
                class="ui-tabs__tab"
                disabled
                role="tab"
                aria-selected="false"
                title="Claude 历史接口待接入"
              >
                <Clock3 :size="13" aria-hidden="true" />
                <span>Claude</span>
                <span class="ui-tabs__count">待接入</span>
              </button>
            </div>
          </div>

          <div class="conversation-import__search">
            <label class="conversation-import__searchbox">
              <Search :size="14" aria-hidden="true" />
              <input
                v-model="query"
                type="search"
                placeholder="搜索 Codex thread"
                aria-label="搜索 Codex thread"
              />
            </label>
            <label class="conversation-import__toggle ui-switch">
              <input v-model="includeArchived" type="checkbox" />
              <span>包含归档</span>
            </label>
          </div>

          <section class="conversation-import__list ui-list" aria-label="Codex thread 列表">
            <div v-if="error" class="conversation-import__notice is-error">{{ error }}</div>
            <div v-else-if="loading" class="conversation-import__notice">
              <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
              <span>正在读取 Codex 历史</span>
            </div>
            <div v-else-if="threads.length === 0" class="conversation-import__notice">
              没有找到 Codex thread
            </div>
            <template v-else>
              <button
                v-for="thread in threads"
                :key="thread.id"
                type="button"
                class="conversation-import__row ui-list-item"
                :class="{ 'is-active': selectedThreadId === thread.id }"
                :title="thread.title"
                @click="selectThread(thread)"
              >
                <span class="conversation-import__row-title">{{ thread.title }}</span>
                <span class="conversation-import__row-time">
                  {{ formatTime(thread.updatedAt ?? thread.createdAt) }}
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

        <section class="conversation-import__preview" aria-label="Codex thread 预览">
          <template v-if="selectedThread">
            <div class="conversation-import__preview-head">
              <div class="conversation-import__preview-heading">
                <div class="conversation-import__preview-title">{{ selectedThread.title }}</div>
                <div class="conversation-import__preview-meta">
                  {{ selectedThreadMeta }}
                </div>
              </div>
              <button
                type="button"
                class="conversation-import__import-button ui-button ui-button--primary"
                :disabled="!selectedThread || importing"
                @click="importSelectedThread"
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
                  这个 thread 暂无可预览事件。
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
          <div v-else class="conversation-import__empty-preview">
            选择一个 Codex thread 后查看摘要并导入。
          </div>
        </section>
      </div>

      <div v-if="importError" class="conversation-import__import-error">
        {{ importError }}
      </div>
    </div>
  </section>
</template>
