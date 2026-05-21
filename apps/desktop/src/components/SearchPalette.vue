<script setup lang="ts">
/**
 * 命令面板风格的搜索浮层：从侧栏左上角的搜索按钮触发。
 *
 * - 模式切换：「文本」走子串匹配并高亮命中区间；「向量」走 TF-IDF 余弦相似度
 *   排序（量纲不同，UI 只用来排序，不展示绝对分值含义）。
 * - 键盘：↑↓ 选项，Enter 打开，Esc 关闭。打开时自动聚焦输入框。
 * - 路由：点击或回车后 router.push 到对应路径，由 SecondaryPanel 关掉浮层。
 */

import { computed, nextTick, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { FileText, Search, Sparkles, X } from "lucide-vue-next";
import {
  searchSessions,
  type SearchMode,
  type SearchResult,
} from "../services/sessionSearch";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const query = ref("");
const mode = ref<SearchMode>("text");
const inputEl = ref<HTMLInputElement | null>(null);
const selectedIdx = ref(0);

const results = computed(() => searchSessions(query.value, mode.value).slice(0, 20));

const router = useRouter();

// 输入或换模式都把游标拨回首项，避免高亮飘到不存在的下标。
watch(results, () => {
  selectedIdx.value = 0;
});

watch(
  () => props.open,
  async (v) => {
    if (v) {
      query.value = "";
      selectedIdx.value = 0;
      await nextTick();
      inputEl.value?.focus();
    }
  },
);

function close() {
  emit("close");
}

function openResult(r: SearchResult) {
  router.push(r.route);
  close();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    close();
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
    if (r) openResult(r);
  }
}

interface Segment {
  text: string;
  mark: boolean;
}

/** 把高亮 ranges 转成「文本段 + 是否高亮」的扁平数组，方便模板渲染。 */
function highlightSegments(text: string, ranges: Array<[number, number]>): Segment[] {
  if (!ranges.length) return [{ text, mark: false }];
  const sorted = [...ranges].sort((a, b) => a[0] - b[0]);
  // 合并重叠区间，否则 mark 会嵌套。
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
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        v-if="open"
        class="search-palette"
        role="dialog"
        aria-modal="true"
        aria-label="搜索会话"
        @click.self="close"
        @keydown="onKeydown"
      >
        <div class="search-palette__card">
          <div class="search-palette__header">
            <Search :size="16" aria-hidden="true" />
            <input
              ref="inputEl"
              v-model="query"
              type="text"
              class="search-palette__input"
              placeholder="搜索会话标题…"
              spellcheck="false"
              @keydown="onKeydown"
            />
            <div class="search-palette__modes" role="tablist">
              <button
                type="button"
                role="tab"
                :aria-selected="mode === 'text'"
                :class="{ 'is-active': mode === 'text' }"
                title="子串匹配"
                @click="mode = 'text'"
              >
                <FileText :size="12" aria-hidden="true" /> 文本
              </button>
              <button
                type="button"
                role="tab"
                :aria-selected="mode === 'vector'"
                :class="{ 'is-active': mode === 'vector' }"
                title="字符 bigram TF-IDF 余弦相似度"
                @click="mode = 'vector'"
              >
                <Sparkles :size="12" aria-hidden="true" /> 向量
              </button>
            </div>
            <button
              type="button"
              class="search-palette__close"
              title="关闭（Esc）"
              aria-label="关闭"
              @click="close"
            >
              <X :size="14" aria-hidden="true" />
            </button>
          </div>

          <ul v-if="results.length" class="search-palette__list" role="listbox">
            <li
              v-for="(r, i) in results"
              :key="r.route"
              class="search-palette__item"
              :class="{ 'is-active': i === selectedIdx }"
              role="option"
              :aria-selected="i === selectedIdx"
              @mouseenter="selectedIdx = i"
              @click="openResult(r)"
            >
              <div class="search-palette__title">
                <template
                  v-for="(seg, j) in highlightSegments(r.title, r.highlights)"
                  :key="j"
                >
                  <mark v-if="seg.mark">{{ seg.text }}</mark>
                  <template v-else>{{ seg.text }}</template>
                </template>
              </div>
              <div class="search-palette__meta">
                <span class="search-palette__scope">{{
                  r.projectName ?? "零散对话"
                }}</span>
                <span class="search-palette__score">{{ r.score.toFixed(2) }}</span>
              </div>
            </li>
          </ul>

          <p v-else-if="query.trim()" class="search-palette__empty">没有匹配</p>
          <p v-else class="search-palette__hint">输入关键词搜索会话标题</p>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
