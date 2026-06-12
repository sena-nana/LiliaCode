import { computed, onBeforeUnmount, ref, watch, type ComputedRef } from "vue";
import type { ChatSlashCommandSearchResult, ChatSlashCommandWorkflow } from "@lilia/contracts";
import { searchSlashCommands } from "../../services/chat";
import { textPart, type MentionRange } from "./composerParts";
import type { useComposerRichInput } from "./useComposerRichInput";

const SLASH_COMMAND_LIMIT = 12;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;

export function readSlashCommandRange(text: string, cursor: number): MentionRange | null {
  const end = Math.min(Math.max(cursor, 0), text.length);
  const prefix = text.slice(0, end);
  const lineStart = Math.max(prefix.lastIndexOf("\n") + 1, 0);
  const beforeLine = text.slice(lineStart, end);
  if (!beforeLine.startsWith("/")) return null;
  if (beforeLine.length > 160 || /\s/.test(beforeLine)) return null;
  return { start: lineStart, end, query: beforeLine.slice(1) };
}

export function useComposerSlashCommands(options: {
  richInput: ComposerRichInput;
  projectCwd: ComputedRef<string | null | undefined>;
  hasPending: ComputedRef<boolean>;
  executeCommand: (workflow: ChatSlashCommandWorkflow) => void;
}) {
  const results = ref<ChatSlashCommandSearchResult[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const activeIndex = ref(0);
  const suppressedKey = ref<string | null>(null);
  let searchSeq = 0;

  const slashRange = computed<MentionRange | null>(() => {
    if (options.hasPending.value) return null;
    return readSlashCommandRange(
      options.richInput.searchText.value,
      options.richInput.inputSelection.value,
    );
  });
  const slashKey = computed(() => {
    const range = slashRange.value;
    return range ? `${range.start}:${range.end}:${range.query}` : null;
  });
  const panelOpen = computed(() =>
    slashRange.value !== null &&
    slashKey.value !== suppressedKey.value
  );
  const activeResult = computed(() => results.value[activeIndex.value] ?? null);

  function clear() {
    results.value = [];
    loading.value = false;
    error.value = null;
    activeIndex.value = 0;
  }

  function noteInputChanged() {
    suppressedKey.value = null;
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
      const cwd = options.projectCwd.value?.trim();
      const nextResults = cwd
        ? await searchSlashCommands(cwd, range.query.trim(), SLASH_COMMAND_LIMIT)
        : [];
      if (seq !== searchSeq) return;
      results.value = nextResults;
      activeIndex.value = 0;
      error.value = cwd ? null : "没有可搜索的项目目录";
    } catch (err) {
      if (seq !== searchSeq) return;
      results.value = [];
      error.value = `命令搜索失败：${String(err)}`;
    } finally {
      if (seq === searchSeq) loading.value = false;
    }
  }

  function moveActive(delta: number) {
    if (results.value.length === 0) return;
    activeIndex.value =
      (activeIndex.value + delta + results.value.length) %
      results.value.length;
  }

  function activateResult(index: number) {
    activeIndex.value = index;
  }

  function selectResult(result: ChatSlashCommandSearchResult | null = activeResult.value) {
    if (!result) return;
    const range = slashRange.value;
    if (!range) return;
    options.richInput.replaceRange(range.start, range.end, [textPart("")]);
    suppressedKey.value = null;
    clear();
    options.executeCommand({
      type: "slash_command",
      commandId: result.command.id,
      source: result.command.source,
      arguments: {},
    });
  }

  function suppressPanel() {
    suppressedKey.value = slashKey.value;
    clear();
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
    if (e.key === "Enter" || e.key === "Tab") {
      if (activeResult.value) {
        e.preventDefault();
        selectResult(activeResult.value);
        return true;
      }
      if (loading.value || slashRange.value?.query.trim()) {
        e.preventDefault();
        return true;
      }
      return false;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      suppressPanel();
      return true;
    }
    return false;
  }

  watch(
    () => [
      panelOpen.value,
      slashRange.value?.start ?? -1,
      slashRange.value?.end ?? -1,
      slashRange.value?.query ?? "",
      options.projectCwd.value ?? "",
    ] as const,
    () => {
      void refresh(slashRange.value);
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
    noteInputChanged,
  };
}
