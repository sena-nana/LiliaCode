import { createLazyLoadState } from "@lilia/ui/utils/lazyLoadState";
import { measurePerfAsync } from "@lilia/ui/diagnostics";

const taskDetailPageContentLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "task-detail.page-content.load",
    async () => (await import("./TaskDetailPageContent.vue")).default,
  )
);

const chatTranscriptLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "task-detail.transcript.load",
    async () => (await import("../../components/chat/ChatTranscript.vue")).default,
  )
);

const chatComposerLoad = createLazyLoadState(() =>
  measurePerfAsync(
    "task-detail.composer.load",
    async () => (await import("../../components/chat/ChatComposer.vue")).default,
  )
);

export function loadTaskDetailPageContent() {
  return taskDetailPageContentLoad.load();
}

export function loadChatTranscript() {
  return chatTranscriptLoad.load();
}

export function loadChatComposer() {
  return chatComposerLoad.load();
}
