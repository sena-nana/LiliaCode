import { computed, onBeforeUnmount, ref, watch, type ComputedRef } from "vue";
import type { ChatConversationReference } from "@lilia/contracts";
import { ensureAllProjectTasksLoaded } from "../../services/tasksStore";
import { searchSessions, type SearchResult } from "../../services/sessionSearch";
import { conversationReferencePart, textPart, type MentionRange } from "./composerParts";
import type { useComposerRichInput } from "./useComposerRichInput";

const CONVERSATION_SEARCH_LIMIT = 12;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;

export function readConversationReferenceRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const start = prefix.lastIndexOf("#");
  if (start < 0) return null;
  const query = text.slice(start + 1, end);
  if (query.length > 240 || /[\n\r]/.test(query)) return null;
  return { start, end, query };
}

function conversationReferenceFromSearchResult(result: SearchResult): ChatConversationReference {
  return {
    taskId: result.taskId,
    title: result.title,
    route: result.route,
    projectId: result.projectId,
    projectName: result.projectName,
  };
}

export function useComposerConversationSearch(options: {
  richInput: ComposerRichInput;
  hasPending: ComputedRef<boolean>;
}) {
  const results = ref<SearchResult[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const activeIndex = ref(0);
  const suppressedKey = ref<string | null>(null);
  const userInteracted = ref(false);
  let searchSeq = 0;

  const referenceRange = computed<MentionRange | null>(() => {
    if (options.hasPending.value) return null;
    return readConversationReferenceRange(
      options.richInput.searchText.value,
      options.richInput.inputSelection.value,
    );
  });
  const referenceKey = computed(() => {
    const range = referenceRange.value;
    return range ? `${range.start}:${range.end}:${range.query}` : null;
  });
  const panelOpen = computed(() =>
    referenceRange.value !== null && referenceKey.value !== suppressedKey.value
  );
  const activeResult = computed(() => results.value[activeIndex.value] ?? null);

  function clear() {
    results.value = [];
    loading.value = false;
    error.value = null;
    activeIndex.value = 0;
    userInteracted.value = false;
  }

  function resetSuppression() {
    suppressedKey.value = null;
  }

  function noteInputChanged() {
    suppressedKey.value = null;
    userInteracted.value = false;
  }

  async function refresh(range: MentionRange | null) {
    const seq = ++searchSeq;
    if (!panelOpen.value || !range) {
      clear();
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      await ensureAllProjectTasksLoaded();
      if (seq !== searchSeq) return;
      const query = range.query.trim();
      results.value = query ? searchSessions(query).slice(0, CONVERSATION_SEARCH_LIMIT) : [];
      activeIndex.value = 0;
      userInteracted.value = false;
    } catch (err) {
      if (seq !== searchSeq) return;
      error.value = `搜索会话失败：${String(err)}`;
    } finally {
      if (seq === searchSeq) loading.value = false;
    }
  }

  function suppressPanel() {
    suppressedKey.value = referenceKey.value;
    clear();
  }

  function selectResult(result: SearchResult | null) {
    if (!result) return;
    const range = referenceRange.value;
    if (!range) return;
    const reference = conversationReferenceFromSearchResult(result);
    const alreadyInserted = options.richInput.hasConversationReference(reference.taskId);
    const replacement = alreadyInserted ? [] : [conversationReferencePart(reference), textPart(" ")];
    options.richInput.replaceRange(range.start, range.end, replacement);
    suppressedKey.value = null;
    clear();
  }

  function moveActive(delta: number) {
    if (results.value.length === 0) return;
    activeIndex.value = (activeIndex.value + delta + results.value.length) % results.value.length;
    userInteracted.value = true;
  }

  function activateResult(index: number) {
    activeIndex.value = index;
    userInteracted.value = true;
  }

  function handleKeydown(event: KeyboardEvent): boolean {
    if (!panelOpen.value) return false;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveActive(1);
      return true;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveActive(-1);
      return true;
    }
    if (event.key === "Enter") {
      const result = activeResult.value;
      const query = referenceRange.value?.query.trim() ?? "";
      if (result && (query.length > 0 || userInteracted.value)) {
        event.preventDefault();
        selectResult(result);
        return true;
      }
      if (query.length > 0 && (loading.value || results.value.length === 0)) {
        event.preventDefault();
        return true;
      }
      return false;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      suppressPanel();
      return true;
    }
    return false;
  }

  watch(
    () => [
      panelOpen.value,
      referenceRange.value?.start ?? -1,
      referenceRange.value?.end ?? -1,
      referenceRange.value?.query ?? "",
    ] as const,
    () => {
      void refresh(referenceRange.value);
    },
    { immediate: true },
  );

  onBeforeUnmount(() => {
    searchSeq += 1;
  });

  return {
    panelOpen,
    results,
    activeIndex,
    loading,
    error,
    handleKeydown,
    selectResult,
    activateResult,
    clear,
    resetSuppression,
    noteInputChanged,
  };
}
