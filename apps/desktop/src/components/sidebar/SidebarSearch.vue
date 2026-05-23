<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Search, X, FileText } from "lucide-vue-next";
import { searchSessions, type SearchResult } from "../../services/sessionSearch";

interface Segment { text: string; mark: boolean; }

const props = defineProps<{
  modelValue?: boolean
}>();

const emit = defineEmits<{
  select: [result: SearchResult]
  'update:modelValue': [value: boolean]
}>();

// ── Search state ──

const active = computed({
  get: () => props.modelValue ?? false,
  set: (val) => emit('update:modelValue', val)
});
const query = ref("");
const inputRef = ref<HTMLInputElement | null>(null);
const selectedIdx = ref(0);

const results = computed<SearchResult[]>(() =>
  searchSessions(query.value, "hybrid").slice(0, 12),
);

watch(results, () => { selectedIdx.value = 0; });

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

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    closeSearch();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (results.value.length) {
      selectedIdx.value = (selectedIdx.value + 1) % results.value.length;
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (results.value.length) {
      selectedIdx.value =
        (selectedIdx.value - 1 + results.value.length) % results.value.length;
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    const r = results.value[selectedIdx.value];
    if (r) {
      emit("select", r);
      closeSearch();
    }
  }
}

// ── Highlight helper ──

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
  <template v-if="!active">
    <button type="button" class="sb-icon-action" title="搜索会话" aria-label="搜索会话" @click="openSearch">
      <Search :size="15" aria-hidden="true" />
    </button>
  </template>

  <template v-else>
    <div class="sb-search">
      <Search :size="14" aria-hidden="true" class="sb-search__leading" />
      <input ref="inputRef" v-model="query" type="text" class="sb-search__input" placeholder="搜索会话…"
        spellcheck="false" @keydown="onKeydown" />
    </div>
    <button type="button" class="sb-icon-action" title="关闭搜索 (Esc)" aria-label="关闭搜索" @click="closeSearch">
      <X :size="15" aria-hidden="true" />
    </button>

    <div class="sb-search-dd" role="listbox">
      <template v-if="results.length">
        <button v-for="(r, i) in results" :key="r.route" type="button" class="sb-search-dd__item"
          :class="{ 'is-active': i === selectedIdx }" role="option" :aria-selected="i === selectedIdx"
          @mouseenter="selectedIdx = i" @click="emit('select', r); closeSearch()">
          <span class="sb-search-dd__title">
            <template v-for="(seg, j) in highlightSegments(r.title, r.highlights)" :key="j">
              <mark v-if="seg.mark">{{ seg.text }}</mark>
              <template v-else>{{ seg.text }}</template>
            </template>
          </span>
          <span v-if="r.projectName" class="sb-search-dd__scope">{{ r.projectName }}</span>
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
