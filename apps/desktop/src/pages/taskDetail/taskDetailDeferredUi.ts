import { nextTick } from "vue";
import type { ChatAttachment, Project, SuggestionItem } from "@lilia/contracts";
import type { Router } from "vue-router";
import type { ConversationSuggestionSources } from "../../services/chat";
import { measurePerfAsync } from "../../utils/perf";

export type SuggestionsStatus = "idle" | "loading" | "empty" | "error";

let suggestionDepsLoad: Promise<{
  getConversationSuggestions: typeof import("../../services/chat").getConversationSuggestions;
  getConversationSuggestionSources: typeof import("../../services/chat").getConversationSuggestionSources;
}> | null = null;

let draftProjectPickerDepsLoad: Promise<{
  createDraftTask: typeof import("../../services/tasksStore").createDraftTask;
  listProjects: typeof import("../../services/projectsStore").listProjects;
}> | null = null;

async function loadSuggestionDeps(detail: string) {
  if (!suggestionDepsLoad) {
    suggestionDepsLoad = measurePerfAsync(
      "task-detail.suggestion-deps.load",
      async () => {
        const {
          getConversationSuggestions,
          getConversationSuggestionSources,
        } = await import("../../services/chat");
        return { getConversationSuggestions, getConversationSuggestionSources };
      },
      { detail },
    ).catch((err) => {
      suggestionDepsLoad = null;
      throw err;
    });
  }
  return suggestionDepsLoad;
}

async function loadDraftProjectPickerDeps(detail: string) {
  if (!draftProjectPickerDepsLoad) {
    draftProjectPickerDepsLoad = measurePerfAsync(
      "task-detail.project-picker-deps.load",
      async () => {
        const [{ createDraftTask }, { listProjects }] = await Promise.all([
          import("../../services/tasksStore"),
          import("../../services/projectsStore"),
        ]);
        return { createDraftTask, listProjects };
      },
      { detail },
    ).catch((err) => {
      draftProjectPickerDepsLoad = null;
      throw err;
    });
  }
  return draftProjectPickerDepsLoad;
}

export async function loadTaskDetailSuggestions(options: {
  detail: string;
  projectId: string | null;
  forceRefresh: boolean;
  shouldLoad: boolean;
  seq: number;
  isCurrentSeq: (seq: number) => boolean;
  setSuggestions: (items: SuggestionItem[]) => void;
  setStatus: (status: SuggestionsStatus) => void;
  setLoadingText: (text: string) => void;
}) {
  const {
    detail,
    projectId,
    forceRefresh,
    shouldLoad,
    seq,
    isCurrentSeq,
    setSuggestions,
    setStatus,
    setLoadingText,
  } = options;
  if (!shouldLoad) {
    setSuggestions([]);
    setStatus("idle");
    setLoadingText("正在寻找灵感");
    return;
  }
  setStatus("loading");
  setLoadingText("正在寻找灵感");
  try {
    const { getConversationSuggestions, getConversationSuggestionSources } = await loadSuggestionDeps(detail);
    const loadingText = suggestionLoadingText(
      await getConversationSuggestionSources(projectId, forceRefresh),
    );
    if (!isCurrentSeq(seq)) return;
    setLoadingText(loadingText);
    const next = await getConversationSuggestions(projectId, forceRefresh);
    if (!isCurrentSeq(seq)) return;
    setSuggestions(next);
    setStatus(next.length > 0 ? "idle" : "empty");
  } catch (err) {
    console.error("[conversation-suggestions] load failed", err);
    if (!isCurrentSeq(seq)) return;
    setSuggestions([]);
    setStatus("error");
  }
}

export async function listTaskDetailDraftProjects(detail: string): Promise<Project[]> {
  const { listProjects } = await loadDraftProjectPickerDeps(detail);
  return listProjects();
}

export async function moveTaskDetailDraftToProject(options: {
  detail: string;
  projectId: string;
  attachments: ChatAttachment[];
  router: Router;
  getDraftSnapshot: () => { content: string };
  restoreDraft: (content: string, attachments: ChatAttachment[]) => void;
}) {
  const { detail, projectId, attachments, router, getDraftSnapshot, restoreDraft } = options;
  const { createDraftTask } = await loadDraftProjectPickerDeps(detail);
  const draft = createDraftTask(projectId);
  const snapshot = getDraftSnapshot();
  const nextAttachments = [...attachments];
  await router.push(`/projects/${projectId}/tasks/${draft.id}`);
  await nextTick();
  restoreDraft(snapshot.content, nextAttachments);
}

function suggestionLoadingText(probe: ConversationSuggestionSources): string {
  const sources = new Set(probe.sources);
  if (sources.has("claude")) return "正在读取 Claude 建议";
  const labels: string[] = [];
  if (sources.has("task")) labels.push("历史任务");
  if (sources.has("github")) labels.push("GitHub 活动");
  if (sources.has("local-git")) {
    labels.push(localGitLoadingLabel(probe.localGit));
  }
  if (labels.length === 0) return "正在寻找灵感";
  return `正在检查${joinSuggestionSourceLabels(labels)}`;
}

function localGitLoadingLabel(localGit: ConversationSuggestionSources["localGit"]): string {
  if (localGit?.hasRecentCommits && localGit.hasChangedFiles) {
    return "本地提交和未提交变更";
  }
  if (localGit?.hasRecentCommits) return "本地提交";
  if (localGit?.hasChangedFiles) return "未提交变更";
  return "本地 Git 上下文";
}

function joinSuggestionSourceLabels(labels: string[]): string {
  if (labels.length <= 1) return labelAfterVerb(labels[0] ?? "");
  if (labels.length === 2) return `${labels[0]}和${labelAfterConjunction(labels[1])}`;
  return `${labels.slice(0, -1).join("、")}和${labelAfterConjunction(labels[labels.length - 1])}`;
}

function labelAfterVerb(label: string): string {
  return /^[A-Za-z]/.test(label) ? ` ${label}` : label;
}

function labelAfterConjunction(label: string): string {
  return /^[A-Za-z]/.test(label) ? ` ${label}` : label;
}
