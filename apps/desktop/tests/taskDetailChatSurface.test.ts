import { fireEvent, render, waitFor } from "@testing-library/vue";
import { defineComponent, h, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatComposerState } from "@lilia/contracts";
import TaskDetailChatSurface from "../src/pages/taskDetail/TaskDetailChatSurface.vue";

const mockTodoFloatSetup = vi.fn();

const MockTodoFloat = defineComponent({
  name: "MockTodoFloat",
  emits: ["set-lilia-goal"],
  setup(_, { emit }) {
    mockTodoFloatSetup();
    return () =>
      h("button", {
        type: "button",
        onClick: () => emit("set-lilia-goal", "新的 Lilia Goal"),
      }, "stub set goal");
  },
});

vi.mock("../src/components/todo/TodoFloat.vue", () => ({
  default: MockTodoFloat,
}));

vi.mock("../src/components/chat/ChatSidebarHost.vue", () => ({
  default: defineComponent({
    name: "MockChatSidebarHost",
    setup() {
      return () => h("div", { "data-testid": "chat-sidebar-host" });
    },
  }),
}));

vi.mock("../src/components/chat/ImageViewer.vue", () => ({
  default: defineComponent({
    name: "MockImageViewer",
    setup() {
      return () => h("div", { "data-testid": "image-viewer" });
    },
  }),
}));

const triggerConversationReference = vi.fn();
const fillSuggestionPrompt = vi.fn();

vi.mock("../src/components/chat/ChatTranscript.vue", () => ({
  default: defineComponent({
    name: "MockChatTranscript",
    setup(_, { slots }) {
      return () => h("div", slots.controls?.());
    },
  }),
}));

vi.mock("../src/components/chat/ChatComposer.vue", () => ({
  default: defineComponent({
    name: "MockChatComposer",
    emits: ["start-lilia-fix-suggestion"],
    setup(_, { emit, expose }) {
      expose({
        fillSuggestionPrompt,
        focusInput: vi.fn(),
        getDraftSnapshot: () => ({ content: "" }),
        triggerConversationReference,
      });
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
}));

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

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
    pendingBranchAnchor: null,
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
  return render(TaskDetailChatSurface, {
    props: surfaceProps(),
  });
}

function renderSurfaceHost() {
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
  return render(Host);
}

describe("TaskDetailChatSurface", () => {
  it("forwards Lilia Goal setting events from TodoFloat", async () => {
    const view = renderSurface();

    const button = await waitFor(() => view.getByRole("button", { name: "stub set goal" }));
    await fireEvent.click(button);

    expect(view.emitted("set-lilia-goal")?.[0]).toEqual(["新的 Lilia Goal"]);
  });

  it("forwards Codex fix suggestion events from the composer", async () => {
    const view = renderSurface();

    const button = await waitFor(() => view.getByRole("button", { name: "stub fix suggestion" }));
    await fireEvent.click(button);

    expect(view.emitted("start-lilia-fix-suggestion")?.[0]).toEqual([
      "优先给最小修复",
      [],
      [],
      { type: "uncommittedChanges" },
    ]);
  });

  it("exposes conversation reference trigger through the surface", async () => {
    const view = renderSurfaceHost();

    await waitFor(() => view.getByRole("button", { name: "stub fix suggestion" }));
    await fireEvent.click(view.getByRole("button", { name: "call surface trigger" }));

    expect(triggerConversationReference).toHaveBeenCalledTimes(1);
  });

  it("cancels deferred TodoFloat render after unmount", async () => {
    vi.useFakeTimers();
    const view = renderSurface();

    view.unmount();
    await vi.runAllTimersAsync();
    await Promise.resolve();

    expect(mockTodoFloatSetup).not.toHaveBeenCalled();
  });

  it("cancels deferred composer activation after unmount", () => {
    const cancelAnimationFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 41));
    vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrame);

    const view = renderSurface();
    view.unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(41);
  });
});
