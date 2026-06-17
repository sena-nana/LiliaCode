import { fireEvent, render } from "@testing-library/vue";
import { defineComponent, h, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { ChatComposerState } from "@lilia/contracts";
import TaskDetailChatSurface from "../src/pages/taskDetail/TaskDetailChatSurface.vue";

const codexComposerState: ChatComposerState = {
  taskId: "task-1",
  backend: "codex",
  model: "gpt-5.5",
  planMode: false,
  goalMode: false,
  permission: "ask",
};

function surfaceProps() {
  return {
    taskId: "task-1",
    projectId: "project-1",
    isPopup: false,
    shouldRenderChat: true,
    isPopupPending: false,
    shouldShowContextLoading: false,
    isContextLoading: false,
    fileDropActive: false,
    timelineEvents: [],
    emptyHeadline: "今天想做什么？",
    isTurnRunning: false,
    projectCwd: "C:/repo",
    contextSearchCwd: "C:/repo",
    activePlanApprovalTurnId: null,
    userSendScrollKey: 0,
    restoreDraftKey: 0,
    restoreDraftContent: "",
    insertDraftTextKey: 0,
    insertDraftTextContent: "",
    pendingAgentActions: [],
    hasBlockingPendingAction: false,
    currentLiliaGoal: null,
    showExpiredPendingActions: false,
    canRetryEvent: () => false,
    composerState: codexComposerState,
    contextUsage: null,
    attachments: [],
    appendAttachmentsToEndKey: 0,
    pendingAsk: null,
    toolConsent: null,
    viewingImage: null,
    suggestions: [],
    suggestionsStatus: "idle" as const,
    suggestionsLoadingText: "正在寻找灵感",
    suggestionsVisible: false,
  };
}

function renderSurface() {
  const triggerConversationReference = vi.fn();
  const view = render(TaskDetailChatSurface, {
    props: surfaceProps(),
    global: {
      stubs: {
        ChatTranscript: defineComponent({
          setup(_, { slots }) {
            return () => h("div", slots.controls?.());
          },
        }),
        ChatComposer: defineComponent({
          emits: ["start-lilia-fix-suggestion"],
          setup(_, { emit, expose }) {
            expose({ triggerConversationReference });
            return () =>
              h("div", [
                h("button", {
                  type: "button",
                  onClick: () => emit(
                    "start-lilia-fix-suggestion",
                    "优先给最小修复",
                    [],
                    [],
                    { type: "uncommittedChanges" },
                  ),
                }, "stub fix suggestion"),
              ]);
          },
        }),
        TodoFloat: defineComponent({
          emits: ["set-lilia-goal"],
          setup(_, { emit }) {
            return () =>
              h("button", {
                type: "button",
                onClick: () => emit("set-lilia-goal", "新的 Lilia Goal"),
              }, "stub set goal");
          },
        }),
        ChatSidebarHost: true,
        ImageViewer: true,
      },
    },
  });
  return { ...view, triggerConversationReference };
}

function renderSurfaceHost() {
  const triggerConversationReference = vi.fn();
  const Host = defineComponent({
    components: { TaskDetailChatSurface },
    setup() {
      const surfaceRef = ref<InstanceType<typeof TaskDetailChatSurface> | null>(null);
      return () => h("div", [
        h(TaskDetailChatSurface, {
          ...surfaceProps(),
          ref: surfaceRef,
        }),
        h("button", {
          type: "button",
          onClick: () => surfaceRef.value?.triggerConversationReference(),
        }, "call surface trigger"),
      ]);
    },
  });
  const view = render(Host, {
    global: {
      stubs: {
        ChatTranscript: defineComponent({
          setup(_, { slots }) {
            return () => h("div", slots.controls?.());
          },
        }),
        ChatComposer: defineComponent({
          setup(_, { expose }) {
            expose({ triggerConversationReference });
            return () => h("div");
          },
        }),
        TodoFloat: true,
        ChatSidebarHost: true,
        ImageViewer: true,
      },
    },
  });
  return { ...view, triggerConversationReference };
}

describe("TaskDetailChatSurface", () => {
  it("forwards Lilia Goal setting events from TodoFloat", async () => {
    const view = renderSurface();

    await fireEvent.click(view.getByRole("button", { name: "stub set goal" }));

    expect(view.emitted("set-lilia-goal")?.[0]).toEqual(["新的 Lilia Goal"]);
  });

  it("forwards Codex fix suggestion events from the composer", async () => {
    const view = renderSurface();

    await fireEvent.click(view.getByRole("button", { name: "stub fix suggestion" }));

    expect(view.emitted("start-lilia-fix-suggestion")?.[0]).toEqual([
      "优先给最小修复",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
  });

  it("exposes conversation reference trigger through the surface", async () => {
    const view = renderSurfaceHost();

    await fireEvent.click(view.getByRole("button", { name: "call surface trigger" }));

    expect(view.triggerConversationReference).toHaveBeenCalledTimes(1);
  });
});
