<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  Check,
  ChevronDown,
  Clock3,
  Code2,
  Import,
  Loader2,
  Search,
} from "lucide-vue-next";
import type { CodexThreadSummary } from "@lilia/contracts";
import {
  attachCodexThread,
  previewCodexThread,
  searchCodexThreads,
} from "../services/chat";
import { ensureOrphansLoaded, ensureProjectTasksLoaded } from "../services/tasksStore";

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

interface PreviewMessage {
  id: string;
  role: string;
  summary: string | null;
}

interface PreviewLite {
  eventCount: number;
  messages: PreviewMessage[];
  hasFullPreview: boolean;
}

const preview = ref<PreviewLite | null>(null);
const fullPreviewEvents = ref<Array<{ id: string; title: string; summary: string | null }>>([]);

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

const previewMessages = computed<PreviewMessage[]>(() => preview.value?.messages ?? []);

function formatTime(value: number | null): string {
  if (!value) return "未知时间";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function toPreviewLite(result: Awaited<ReturnType<typeof previewCodexThread>>): PreviewLite {
  return {
    eventCount: result.eventCount,
    messages: (result.messages ?? []).map((message) => ({
      id: message.id,
      role: message.role === "user" ? "用户" : "Codex",
      summary: message.summary,
    })),
    hasFullPreview: result.hasFullPreview === true,
  };
}

function toFullPreviewEvents(result: Awaited<ReturnType<typeof previewCodexThread>>) {
  return result.events.map((event) => ({
    id: event.id,
    title: event.title,
    summary: event.summary,
  }));
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
    preview.value = null;
    fullPreviewEvents.value = [];
    importError.value = "";
    void loadThreads();
  }, 240);
}

async function selectThread(thread: CodexThreadSummary) {
  selectedThreadId.value = thread.id;
  preview.value = null;
  fullPreviewEvents.value = [];
  previewError.value = "";
  fullPreviewError.value = "";
  importError.value = "";
  previewLoading.value = true;
  const seq = ++previewSeq;
  fullPreviewSeq += 1;
  try {
    const result = await previewCodexThread({ threadId: thread.id, detail: "lite" });
    if (seq === previewSeq) preview.value = toPreviewLite(result);
  } catch (err) {
    if (seq === previewSeq) previewError.value = String(err);
  } finally {
    if (seq === previewSeq) previewLoading.value = false;
  }
}

async function loadFullPreview() {
  const thread = selectedThread.value;
  if (!thread || fullPreviewLoading.value || fullPreviewEvents.value.length > 0) return;
  fullPreviewLoading.value = true;
  fullPreviewError.value = "";
  const seq = ++fullPreviewSeq;
  try {
    const result = await previewCodexThread({ threadId: thread.id, detail: "full" });
    if (seq === fullPreviewSeq) fullPreviewEvents.value = toFullPreviewEvents(result);
  } catch (err) {
    if (seq === fullPreviewSeq) fullPreviewError.value = String(err);
  } finally {
    if (seq === fullPreviewSeq) fullPreviewLoading.value = false;
  }
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
    <div class="page-header conversation-import__page-header">
      <div>
        <h1>导入对话</h1>
        <p>从已有 Claude / Codex 历史中选择一个对话，导入后继续处理。</p>
      </div>
      <button
        type="button"
        class="primary"
        :disabled="!selectedThread || importing"
        @click="importSelectedThread"
      >
        <Loader2 v-if="importing" :size="14" class="is-spinning" aria-hidden="true" />
        <Check v-else :size="14" aria-hidden="true" />
        <span>{{ importing ? "导入中…" : importTargetLabel }}</span>
      </button>
    </div>

    <div class="conversation-import">
      <div class="conversation-import__source-bar">
        <div class="conversation-import__tabs" role="tablist" aria-label="导入来源">
          <button
            type="button"
            class="conversation-import__tab is-active"
            role="tab"
            aria-selected="true"
          >
            <Code2 :size="13" aria-hidden="true" />
            <span>Codex</span>
          </button>
          <button
            type="button"
            class="conversation-import__tab"
            disabled
            role="tab"
            aria-selected="false"
            title="Claude 历史接口待接入"
          >
            <Clock3 :size="13" aria-hidden="true" />
            <span>Claude</span>
            <span class="conversation-import__tab-badge">待接入</span>
          </button>
        </div>
        <Import :size="16" aria-hidden="true" />
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
        <label class="conversation-import__toggle">
          <input v-model="includeArchived" type="checkbox" />
          <span>包含归档</span>
        </label>
      </div>

      <div class="conversation-import__content">
        <section class="conversation-import__list" aria-label="Codex thread 列表">
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
              class="conversation-import__row"
              :class="{ 'is-active': selectedThreadId === thread.id }"
              :title="thread.title"
              @click="selectThread(thread)"
            >
              <span class="conversation-import__row-title">{{ thread.title }}</span>
              <span class="conversation-import__row-meta">
                {{ formatTime(thread.updatedAt ?? thread.createdAt) }}
                <span v-if="thread.model"> · {{ thread.model }}</span>
              </span>
              <span v-if="thread.preview" class="conversation-import__row-preview">
                {{ thread.preview }}
              </span>
            </button>
          </template>
          <button
            v-if="nextCursor"
            type="button"
            class="conversation-import__more"
            :disabled="loadingMore"
            @click="loadThreads(nextCursor)"
          >
            <Loader2 v-if="loadingMore" :size="14" class="is-spinning" aria-hidden="true" />
            <ChevronDown v-else :size="14" aria-hidden="true" />
            <span>加载更多</span>
          </button>
        </section>

        <section class="conversation-import__preview" aria-label="Codex thread 预览">
          <template v-if="selectedThread">
            <div class="conversation-import__preview-head">
              <div class="conversation-import__preview-title">{{ selectedThread.title }}</div>
              <div class="conversation-import__preview-meta">
                {{ formatTime(selectedThread.updatedAt ?? selectedThread.createdAt) }}
                <span v-if="preview?.eventCount"> · {{ preview.eventCount }} 条事件</span>
                <span v-if="selectedThread.status"> · {{ selectedThread.status }}</span>
              </div>
            </div>

            <div v-if="previewLoading" class="conversation-import__notice">
              <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
              <span>正在生成预览</span>
            </div>
            <div v-else-if="previewError" class="conversation-import__notice is-error">
              {{ previewError }}
            </div>
            <div v-else class="conversation-import__messages">
              <div
                v-for="event in previewMessages"
                :key="event.id"
                class="conversation-import__message"
              >
                <span>{{ event.role }}</span>
                <p>{{ event.summary }}</p>
              </div>
              <div v-if="previewMessages.length === 0" class="conversation-import__notice">
                这个 thread 暂无可预览消息
              </div>
              <button
                v-if="preview?.hasFullPreview"
                type="button"
                class="conversation-import__full-preview"
                :disabled="fullPreviewLoading"
                @click="loadFullPreview"
              >
                <Loader2 v-if="fullPreviewLoading" :size="13" class="is-spinning" aria-hidden="true" />
                <ChevronDown v-else :size="13" aria-hidden="true" />
                <span>{{ fullPreviewEvents.length ? "完整详情已加载" : "展开完整详情" }}</span>
              </button>
              <div v-if="fullPreviewError" class="conversation-import__notice is-error">
                {{ fullPreviewError }}
              </div>
              <div v-if="fullPreviewEvents.length" class="conversation-import__full-events">
                <div
                  v-for="event in fullPreviewEvents"
                  :key="event.id"
                  class="conversation-import__event"
                >
                  <span>{{ event.title }}</span>
                  <p>{{ event.summary }}</p>
                </div>
              </div>
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
