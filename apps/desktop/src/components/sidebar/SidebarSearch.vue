<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { FileText, Search, X } from "lucide-vue-next";
import { searchSessions, type SearchResult } from "../../services/sessionSearch";
import { ensureAllProjectTasksLoaded } from "../../services/tasksStore";
import SearchDropdown from "../SearchDropdown.vue";

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
const inputRef = ref<{ focus: () => void } | null>(null);
const selectedIdx = ref(0);
const hydrating = ref(false);

const results = computed<SearchResult[]>(() =>
  searchSessions(query.value).slice(0, 12),
);

watch(results, () => {
  selectedIdx.value = 0;
});

async function openSearch() {
  active.value = true;
  query.value = "";
  selectedIdx.value = 0;
  void hydrateCorpus();
  await nextTick();
  inputRef.value?.focus();
}

async function hydrateCorpus() {
  if (hydrating.value) return;
  hydrating.value = true;
  try {
    await ensureAllProjectTasksLoaded();
  } finally {
    hydrating.value = false;
  }
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
    <SearchDropdown
      ref="inputRef"
      v-model="query"
      class="sidebar-search-dropdown"
      placeholder="搜索会话…"
      :spellcheck="false"
      @keydown="onKeydown"
    >
      <template #leading>
        <Search :size="14" aria-hidden="true" class="search-dropdown__leading" />
      </template>
      <template #trailing>
        <button
          type="button"
          class="search-dropdown__action"
          title="关闭搜索 (Esc)"
          aria-label="关闭搜索"
          @click="closeSearch"
        >
          <X :size="13" aria-hidden="true" />
        </button>
      </template>

      <template #default="{ highlightRangeSegments }">
        <template v-if="results.length">
          <button
            v-for="(result, index) in results"
            :key="result.route"
            type="button"
            class="search-dropdown__item"
            :class="{ 'is-active': index === selectedIdx }"
            role="option"
            :aria-selected="index === selectedIdx"
            @mouseenter="selectedIdx = index"
            @click="selectResult(result)"
          >
            <span class="search-dropdown__title">
              <template
                v-for="(segment, segmentIndex) in highlightRangeSegments(result.title, result.highlights)"
                :key="segmentIndex"
              >
                <mark v-if="segment.mark">{{ segment.text }}</mark>
                <template v-else>{{ segment.text }}</template>
              </template>
            </span>
            <span v-if="result.projectName" class="search-dropdown__scope">
              {{ result.projectName }}
            </span>
          </button>
        </template>
        <p v-else-if="query.trim()" class="search-dropdown__empty">没有匹配</p>
        <p v-else class="search-dropdown__hint">
          <FileText :size="11" aria-hidden="true" />
          输入关键词
        </p>
      </template>
    </SearchDropdown>
  </template>
</template>
