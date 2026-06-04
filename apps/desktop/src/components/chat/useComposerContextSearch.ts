import { computed, onBeforeUnmount, ref, watch, type ComputedRef } from "vue";
import type { ChatAttachment, ChatContextSearchResult } from "@lilia/contracts";
import { describeAttachments, searchContextAttachments } from "../../services/chat";
import { attachmentPart, textPart, type MentionRange } from "./composerParts";
import type { useComposerRichInput } from "./useComposerRichInput";

const CONTEXT_SEARCH_LIMIT = 12;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;

export function isAbsolutePathLike(value: string): boolean {
  const trimmed = value.trim();
  return /^[a-zA-Z]:[\\/]/.test(trimmed) ||
    trimmed.startsWith("\\\\") ||
    trimmed.startsWith("/");
}

export function readMentionRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const start = prefix.lastIndexOf("@");
  if (start < 0) return null;
  const query = text.slice(start + 1, end);
  if (query.length > 240 || /[\n\r]/.test(query)) return null;
  if (/\s/.test(query) && !isAbsolutePathLike(query)) return null;
  return { start, end, query };
}

export function isContextPathQueryLike(value: string): boolean {
  return value.includes("/") || value.includes("\\");
}

export function compactPathLabel(value: string): string {
  return value.replace(/\\/g, "/");
}

export function contextInlinePath(result: ChatContextSearchResult): string | null {
  const path = compactPathLabel(result.relativePath);
  return path && path !== result.attachment.name ? path : null;
}

export function useComposerContextSearch(options: {
  richInput: ComposerRichInput;
  projectCwd: ComputedRef<string | null | undefined>;
  hasPending: ComputedRef<boolean>;
  addContextAttachment: (attachment: ChatAttachment) => void;
}) {
  const results = ref<ChatContextSearchResult[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const missingPath = ref<string | null>(null);
  const activeIndex = ref(0);
  const suppressedKey = ref<string | null>(null);
  const noMatchSuppression = ref<{
    start: number;
    query: string;
  } | null>(null);
  const userInteracted = ref(false);
  let searchSeq = 0;

  const mentionRange = computed<MentionRange | null>(() => {
    if (options.hasPending.value) return null;
    return readMentionRange(options.richInput.searchText.value, options.richInput.inputSelection.value);
  });
  const mentionKey = computed(() => {
    const range = mentionRange.value;
    return range ? `${range.start}:${range.end}:${range.query}` : null;
  });
  const panelOpen = computed(() =>
    mentionRange.value !== null &&
    !isAutoSuppressed(mentionRange.value) &&
    mentionKey.value !== suppressedKey.value
  );
  const activeResult = computed(() => results.value[activeIndex.value] ?? null);
  const showMissingPath = computed(() =>
    panelOpen.value &&
    !loading.value &&
    results.value.length === 0 &&
    !!missingPath.value
  );

  function isAutoSuppressed(range: MentionRange): boolean {
    const suppression = noMatchSuppression.value;
    if (!suppression || suppression.start !== range.start) return false;
    if (range.query.trim().length === 0 || isContextPathQueryLike(range.query)) return false;
    return range.query.length > suppression.query.length &&
      range.query.startsWith(suppression.query);
  }

  function clear() {
    results.value = [];
    loading.value = false;
    error.value = null;
    missingPath.value = null;
    activeIndex.value = 0;
    userInteracted.value = false;
  }

  function resetSuppression() {
    suppressedKey.value = null;
    noMatchSuppression.value = null;
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
    const query = range.query.trim();
    loading.value = true;
    error.value = null;
    missingPath.value = null;
    try {
      let nextResults: ChatContextSearchResult[] = [];
      let nextMissingPath: string | null = null;
      let nextError: string | null = null;
      if (isAbsolutePathLike(query)) {
        const [attachment] = await describeAttachments([query]);
        if (attachment?.exists !== false) {
          nextResults = [{
            attachment,
            relativePath: compactPathLabel(attachment.path),
            matchedBy: "path",
          }];
        } else if (attachment) {
          nextMissingPath = attachment.path;
        } else {
          nextMissingPath = query;
        }
      } else {
        const cwd = options.projectCwd.value?.trim();
        if (!cwd) {
          nextError = "没有可搜索的项目目录";
        } else {
          nextResults = await searchContextAttachments(cwd, query, CONTEXT_SEARCH_LIMIT);
        }
      }
      if (seq !== searchSeq) return;
      results.value = nextResults;
      missingPath.value = nextMissingPath;
      error.value = nextError;
      activeIndex.value = 0;
      userInteracted.value = false;
      noMatchSuppression.value = nextResults.length === 0 &&
        !nextMissingPath &&
        !nextError &&
        query.length > 0 &&
        !isContextPathQueryLike(range.query)
        ? { start: range.start, query: range.query }
        : null;
    } catch (err) {
      if (seq !== searchSeq) return;
      error.value = `搜索失败：${String(err)}`;
      noMatchSuppression.value = null;
    } finally {
      if (seq === searchSeq) {
        loading.value = false;
      }
    }
  }

  function suppressPanel() {
    suppressedKey.value = mentionKey.value;
    clear();
  }

  function replaceMentionWithText(range: MentionRange, text: string) {
    options.richInput.replaceRange(range.start, range.end, [textPart(text)]);
    suppressedKey.value = null;
  }

  function selectResult(result: ChatContextSearchResult | null) {
    if (!result || result.attachment.exists === false) return;
    const range = mentionRange.value;
    if (!range) return;
    const alreadyInserted = options.richInput.hasAttachmentPath(result.attachment.path);
    const replacement = alreadyInserted ? [] : [attachmentPart(result.attachment), textPart(" ")];
    options.richInput.replaceRange(range.start, range.end, replacement);
    suppressedKey.value = null;
    clear();
    if (!alreadyInserted) options.addContextAttachment(result.attachment);
  }

  function navigateDirectory(result: ChatContextSearchResult | null): boolean {
    if (!result || result.attachment.exists === false || result.attachment.kind !== "directory") {
      return false;
    }
    const range = mentionRange.value;
    if (!range) return false;
    const relativePath = compactPathLabel(result.relativePath);
    const nextQuery = relativePath.endsWith("/") ? relativePath : `${relativePath}/`;
    replaceMentionWithText(range, `@${nextQuery}`);
    userInteracted.value = true;
    return true;
  }

  function parentContextQuery(query: string): string | null {
    const normalized = compactPathLabel(query.trim()).replace(/^\.\//, "");
    if (!normalized.includes("/") && !normalized.endsWith("/")) return null;
    const current = normalized.replace(/\/+$/, "");
    if (!current) return null;
    const slash = current.lastIndexOf("/");
    return slash < 0 ? "" : `${current.slice(0, slash)}/`;
  }

  function retreatDirectory(): boolean {
    const range = mentionRange.value;
    if (!range) return false;
    const parentQuery = parentContextQuery(range.query);
    if (parentQuery === null) return false;
    replaceMentionWithText(range, `@${parentQuery}`);
    userInteracted.value = true;
    return true;
  }

  function moveActive(delta: number) {
    if (results.value.length === 0) return;
    activeIndex.value =
      (activeIndex.value + delta + results.value.length) %
      results.value.length;
    userInteracted.value = true;
  }

  function activateResult(index: number) {
    activeIndex.value = index;
    userInteracted.value = true;
  }

  function handleKeydown(e: KeyboardEvent): boolean {
    if (!panelOpen.value) return false;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      moveActive(1);
      return true;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      moveActive(-1);
      return true;
    }
    if (e.key === "Enter") {
      const result = activeResult.value;
      const query = mentionRange.value?.query.trim() ?? "";
      if (result && (query.length > 0 || userInteracted.value)) {
        e.preventDefault();
        selectResult(result);
        return true;
      }
      if (query.length > 0 &&
        (loading.value || showMissingPath.value || results.value.length === 0)) {
        e.preventDefault();
        return true;
      }
      return false;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      userInteracted.value = true;
      navigateDirectory(activeResult.value);
      return true;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      if (retreatDirectory()) return true;
      suppressPanel();
      return true;
    }
    return false;
  }

  watch(
    () => [
      panelOpen.value,
      mentionRange.value?.start ?? -1,
      mentionRange.value?.end ?? -1,
      mentionRange.value?.query ?? "",
      options.projectCwd.value ?? "",
    ] as const,
    () => {
      void refresh(mentionRange.value);
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
    missingPath,
    showMissingPath,
    handleKeydown,
    selectResult,
    activateResult,
    clear,
    resetSuppression,
    noteInputChanged,
  };
}
