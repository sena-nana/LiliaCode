import { computed, onScopeDispose, ref, watch, type ComputedRef } from "vue";
import {
  CHAT_WORKFLOW_SLASH_COMMANDS,
  createChatSlashCommandWorkflow,
  type ChatWorkflowSlashKind,
  type ChatSlashCommandSearchResult,
  type ChatSlashCommandWorkflow,
  type LiliaReviewTarget,
} from "@lilia/contracts";
import { measurePerfAsync } from "../../utils/perf";
import { createLazyLoadState } from "../../utils/lazyLoadState";
import { textPart, type MentionRange } from "./composerParts";
import { readSlashCommandRange } from "./composerTriggerRanges";
import type { useComposerRichInput } from "./useComposerRichInput";

const SLASH_COMMAND_LIMIT = 12;

type ComposerRichInput = ReturnType<typeof useComposerRichInput>;
export type ComposerWorkflowSlashKind = ChatWorkflowSlashKind;
type SlashCommandDeps = {
  searchSlashCommands: typeof import("../../services/chat").searchSlashCommands;
};

const slashCommandDepsLoad = createLazyLoadState<SlashCommandDeps>(() =>
  measurePerfAsync(
    "chat-composer.slash-search.load",
    async () => {
      const { searchSlashCommands } = await import("../../services/chat");
      return { searchSlashCommands };
    },
  )
);

async function loadSlashCommandDeps(): Promise<SlashCommandDeps> {
  return slashCommandDepsLoad.load();
}

export interface ComposerSlashCommandItem {
  kind: "command" | "workflow";
  command: ChatSlashCommandSearchResult["command"];
  matchedBy: ChatSlashCommandSearchResult["matchedBy"];
  workflowKind?: ComposerWorkflowSlashKind;
}

export interface ComposerSlashTargetItem {
  id: "uncommitted" | "branch" | "commit";
  label: string;
  hint: string;
}

const WORKFLOW_COMMANDS: ComposerSlashCommandItem[] = CHAT_WORKFLOW_SLASH_COMMANDS.map((item) => ({
  kind: "workflow",
  workflowKind: item.kind,
  matchedBy: "name",
  command: item.command,
}));

const TARGET_ITEMS: ComposerSlashTargetItem[] = [
  { id: "uncommitted", label: "未提交改动", hint: "当前工作区未提交改动" },
  { id: "branch", label: "对比分支...", hint: "输入要对比的分支" },
  { id: "commit", label: "指定提交...", hint: "输入要审查的提交" },
];

export function useComposerSlashCommands(options: {
  richInput: ComposerRichInput;
  projectCwd: ComputedRef<string | null | undefined>;
  hasPending: ComputedRef<boolean>;
  executeCommand: (workflow: ChatSlashCommandWorkflow) => void;
  startWorkflow: (kind: ComposerWorkflowSlashKind, target: LiliaReviewTarget) => void;
}) {
  const results = ref<ComposerSlashCommandItem[]>([]);
  const activeWorkflowKind = ref<ComposerWorkflowSlashKind | null>(null);
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
    (activeWorkflowKind.value !== null || slashRange.value !== null) &&
    slashKey.value !== suppressedKey.value
  );
  const activeResult = computed(() => results.value[activeIndex.value] ?? null);
  const activeTarget = computed(() => TARGET_ITEMS[activeIndex.value] ?? null);

  function clear() {
    results.value = [];
    activeWorkflowKind.value = null;
    loading.value = false;
    error.value = null;
    activeIndex.value = 0;
  }

  function noteInputChanged() {
    suppressedKey.value = null;
    activeWorkflowKind.value = null;
  }

  function localWorkflowResults(query: string): ComposerSlashCommandItem[] {
    const normalized = query.trim().toLowerCase();
    return WORKFLOW_COMMANDS
      .map((item) => {
        if (!normalized || item.command.name.includes(normalized)) {
          return { ...item, matchedBy: "name" as const };
        }
        if (item.command.title.toLowerCase().includes(normalized)) {
          return { ...item, matchedBy: "title" as const };
        }
        if (item.command.description.toLowerCase().includes(normalized)) {
          return { ...item, matchedBy: "description" as const };
        }
        return null;
      })
      .filter((item): item is ComposerSlashCommandItem => item !== null);
  }

  async function refresh(range: MentionRange | null) {
    const seq = ++searchSeq;
    if (activeWorkflowKind.value) return;
    if (!panelOpen.value || !range) {
      clear();
      return;
    }
    loading.value = true;
    error.value = null;
    try {
      const deps = await loadSlashCommandDeps();
      const cwd = options.projectCwd.value?.trim();
      const query = range.query.trim();
      const workflowResults = localWorkflowResults(query);
      const backendResults = cwd
        ? await measurePerfAsync(
          "chat-composer.slash-search.query",
          () => deps.searchSlashCommands(cwd, query, SLASH_COMMAND_LIMIT),
        )
        : [];
      if (seq !== searchSeq) return;
      results.value = [
        ...workflowResults,
        ...backendResults.map((result): ComposerSlashCommandItem => ({
          kind: "command",
          command: result.command,
          matchedBy: result.matchedBy,
        })),
      ].slice(0, SLASH_COMMAND_LIMIT);
      activeIndex.value = 0;
      error.value = cwd || workflowResults.length > 0 ? null : "没有可搜索的项目目录";
    } catch (err) {
      if (seq !== searchSeq) return;
      results.value = localWorkflowResults(range.query.trim());
      error.value = `命令搜索失败：${String(err)}`;
    } finally {
      if (seq === searchSeq) loading.value = false;
    }
  }

  function moveActive(delta: number) {
    const count = activeWorkflowKind.value ? TARGET_ITEMS.length : results.value.length;
    if (count === 0) return;
    activeIndex.value =
      (activeIndex.value + delta + count) % count;
  }

  function activateResult(index: number) {
    activeIndex.value = index;
  }

  function selectResult(result: ComposerSlashCommandItem | null = activeResult.value) {
    if (!result) return;
    const range = slashRange.value;
    if (!range) return;
    if (result.kind === "workflow" && result.workflowKind) {
      activeWorkflowKind.value = result.workflowKind;
      results.value = [];
      activeIndex.value = 0;
      loading.value = false;
      error.value = null;
      return;
    }
    options.richInput.replaceRange(range.start, range.end, [textPart("")]);
    suppressedKey.value = null;
    clear();
    options.executeCommand(createChatSlashCommandWorkflow({
      commandId: result.command.id,
      source: result.command.source,
      arguments: {},
    }));
  }

  function targetFromItem(item: ComposerSlashTargetItem): LiliaReviewTarget | null {
    if (item.id === "uncommitted") return { type: "uncommittedChanges" };
    if (item.id === "branch") {
      const branch = window.prompt("对比分支")?.trim();
      return branch ? { type: "baseBranch", branch } : null;
    }
    const sha = window.prompt("指定提交")?.trim();
    return sha ? { type: "commit", sha } : null;
  }

  function selectTarget(item: ComposerSlashTargetItem | null = activeTarget.value) {
    const kind = activeWorkflowKind.value;
    if (!kind || !item) return;
    const target = targetFromItem(item);
    if (!target) return;
    const range = slashRange.value;
    if (range) options.richInput.replaceRange(range.start, range.end, [textPart("")]);
    suppressedKey.value = null;
    clear();
    options.startWorkflow(kind, target);
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
      if (activeWorkflowKind.value) {
        e.preventDefault();
        selectTarget(activeTarget.value);
        return true;
      }
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
      if (activeWorkflowKind.value) {
        activeWorkflowKind.value = null;
        activeIndex.value = 0;
        void refresh(slashRange.value);
        return true;
      }
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

  onScopeDispose(() => {
    searchSeq += 1;
  });

  return {
    panelOpen,
    results,
    targetItems: TARGET_ITEMS,
    activeWorkflowKind,
    activeIndex,
    loading,
    error,
    handleKeydown,
    selectResult,
    selectTarget,
    activateResult,
    clear,
    noteInputChanged,
  };
}
