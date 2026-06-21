import { nextTick } from "vue";
import {
  DEFAULT_SUGGESTION_LOADING_TEXT,
  conversationSuggestionLoadingText,
  type ChatAttachment,
  type Project,
  type SuggestionItem,
  type SuggestionStatus,
} from "@lilia/contracts";
import type { Router } from "vue-router";
import { measurePerfAsync } from "../../utils/perf";

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
  setStatus: (status: SuggestionStatus) => void;
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
    setLoadingText(DEFAULT_SUGGESTION_LOADING_TEXT);
    return;
  }
  setStatus("loading");
  setLoadingText(DEFAULT_SUGGESTION_LOADING_TEXT);
  try {
    const { getConversationSuggestions, getConversationSuggestionSources } = await loadSuggestionDeps(detail);
    const loadingText = conversationSuggestionLoadingText(
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
  isSourceCurrent: () => boolean;
  isTargetCurrent: (taskId: string) => boolean;
  getDraftSnapshot: () => { content: string };
  restoreDraft: (content: string, attachments: ChatAttachment[]) => void;
}) {
  const {
    detail,
    projectId,
    attachments,
    router,
    isSourceCurrent,
    isTargetCurrent,
    getDraftSnapshot,
    restoreDraft,
  } = options;
  const { createDraftTask } = await loadDraftProjectPickerDeps(detail);
  if (!isSourceCurrent()) return;
  const draft = createDraftTask(projectId);
  const snapshot = getDraftSnapshot();
  const nextAttachments = [...attachments];
  await router.push(`/projects/${projectId}/tasks/${draft.id}`);
  await nextTick();
  if (!isTargetCurrent(draft.id)) return;
  restoreDraft(snapshot.content, nextAttachments);
}
