import { computed, nextTick, ref, watch } from "vue";
import { searchSessions, type SearchResult } from "../services/sessionSearch";

export interface Segment {
  text: string;
  mark: boolean;
}

export function highlightSegments(text: string, ranges: Array<[number, number]>): Segment[] {
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

export function useSearch() {
  const active = ref(false);
  const query = ref("");
  const inputRef = ref<HTMLInputElement | null>(null);
  const selectedIdx = ref(0);

  const results = computed<SearchResult[]>(() =>
    searchSessions(query.value, "hybrid").slice(0, 12),
  );

  watch(results, () => {
    selectedIdx.value = 0;
  });

  async function open() {
    active.value = true;
    query.value = "";
    selectedIdx.value = 0;
    await nextTick();
    inputRef.value?.focus();
  }

  function close() {
    active.value = false;
    query.value = "";
    selectedIdx.value = 0;
  }

  function onKeydown(e: KeyboardEvent, onSelect: (r: SearchResult) => void) {
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
          (selectedIdx.value - 1 + results.value.length) %
          results.value.length;
      }
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results.value[selectedIdx.value];
      if (r) onSelect(r);
    }
  }

  return {
    active,
    query,
    inputRef,
    selectedIdx,
    results,
    open,
    close,
    onKeydown,
  };
}
