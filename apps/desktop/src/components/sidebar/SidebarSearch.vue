<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { FileText, Search, X } from "lucide-vue-next";
import { searchSessions, type SearchResult } from "../../services/sessionSearch";

interface Segment {
  text: string;
  mark: boolean;
}

const props = defineProps<{
  modelValue?: boolean;
}>();

const emit = defineEmits<{
  select: [result: SearchResult];
  "update:modelValue": [value: boolean];
}>();

const active = computed({
  get: () => props.modelValue ?? false,
  set: (value) => emit("update:modelValue", value),
});
const query = ref("");
const inputRef = ref<HTMLInputElement | null>(null);
const selectedIdx = ref(0);

const results = computed<SearchResult[]>(() =>
  searchSessions(query.value, "hybrid").slice(0, 12),
);

watch(results, () => {
  selectedIdx.value = 0;
});

async function openSearch() {
  active.value = true;
  query.value = "";
  selectedIdx.value = 0;
  await nextTick();
  inputRef.value?.focus();
}

function closeSearch() {
  active.value = false;
  query.value = "";
  selectedIdx.value = 0;
}

function selectResult(result: SearchResult) {
  emit("select", result);
  closeSearch();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    closeSearch();
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    if (results.value.length) {
      selectedIdx.value = (selectedIdx.value + 1) % results.value.length;
    }
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    if (results.value.length) {
      selectedIdx.value =
        (selectedIdx.value - 1 + results.value.length) % results.value.length;
    }
  } else if (event.key === "Enter") {
    event.preventDefault();
    const result = results.value[selectedIdx.value];
    if (result) selectResult(result);
  }
}

function highlightSegments(
  text: string,
  ranges: Array<[number, number]>,
): Segment[] {
  if (!ranges.length) return [{ text, mark: false }];
  const sorted = [...ranges].sort((a, b) => a[0] - b[0]);
  const merged: Array<[number, number]> = [];
  for (const [start, end] of sorted) {
    const last = merged[merged.length - 1];
    if (last && start <= last[1]) {
      last[1] = Math.max(last[1], end);
    } else {
      merged.push([start, end]);
    }
  }

  const segments: Segment[] = [];
  let cursor = 0;
  for (const [start, end] of merged) {
    if (cursor < start) {
      segments.push({ text: text.slice(cursor, start), mark: false });
    }
    segments.push({ text: text.slice(start, end), mark: true });
    cursor = end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), mark: false });
  }
  return segments;
}
</script>

<template>
  <template v-if="!active">
    <button
      type="button"
      class="sb-icon-action"
      title="搜索会话"
      aria-label="搜索会话"
      @click="openSearch"
    >
      <Search :size="15" aria-hidden="true" />
    </button>
  </template>

  <template v-else>
    <div class="sb-search">
      <Search :size="14" aria-hidden="true" class="sb-search__leading" />
      <input
        ref="inputRef"
        v-model="query"
        type="text"
        class="sb-search__input"
        placeholder="搜索会话…"
        spellcheck="false"
        @keydown="onKeydown"
      />
    </div>
    <button
      type="button"
      class="sb-icon-action"
      title="关闭搜索 (Esc)"
      aria-label="关闭搜索"
      @click="closeSearch"
    >
      <X :size="15" aria-hidden="true" />
    </button>

    <div class="sb-search-dd" role="listbox">
      <template v-if="results.length">
        <button
          v-for="(result, index) in results"
          :key="result.route"
          type="button"
          class="sb-search-dd__item"
          :class="{ 'is-active': index === selectedIdx }"
          role="option"
          :aria-selected="index === selectedIdx"
          @mouseenter="selectedIdx = index"
          @click="selectResult(result)"
        >
          <span class="sb-search-dd__title">
            <template
              v-for="(segment, segmentIndex) in highlightSegments(result.title, result.highlights)"
              :key="segmentIndex"
            >
              <mark v-if="segment.mark">{{ segment.text }}</mark>
              <template v-else>{{ segment.text }}</template>
            </template>
          </span>
          <span v-if="result.projectName" class="sb-search-dd__scope">
            {{ result.projectName }}
          </span>
        </button>
      </template>
      <p v-else-if="query.trim()" class="sb-search-dd__empty">没有匹配</p>
      <p v-else class="sb-search-dd__hint">
        <FileText :size="11" aria-hidden="true" />
        输入关键词
      </p>
    </div>
  </template>
</template>
