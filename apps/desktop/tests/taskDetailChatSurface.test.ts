import { fireEvent, render } from "@testing-library/vue";
import { defineComponent, h } from "vue";
import { describe, expect, it } from "vitest";
import type { ChatComposerState } from "@lilia/contracts";
import TaskDetailChatSurface from "../src/pages/taskDetail/TaskDetailChatSurface.vue";

const codexComposerState: ChatComposerState = {
  taskId: "task-1",
  backend: "codex",
  model: "gpt-5.5",
  planMode: false,
  permission: "ask",
};

function renderSurface() {
  return render(TaskDetailChatSurface, {
    props: {
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
      attachments: [],
      appendAttachmentsToEndKey: 0,
      pendingAsk: null,
      toolConsent: null,
      viewingImage: null,
      suggestions: [],
      suggestionsVisible: false,
    },
    global: {
      stubs: {
        ChatTranscript: defineComponent({
          setup(_, { slots }) {
            return () => h("div", slots.controls?.());
          },
        }),
        ChatComposer: defineComponent({
          emits: ["start-lilia-fix-suggestion", "apply-lilia-provider-settings"],
          setup(_, { emit }) {
            return () =>
              h("div", [
                h("button", {
                  type: "button",
                  onClick: () => emit(
                    "start-lilia-fix-suggestion",
                    "优先给最小修复",
                    [],
                    { type: "uncommittedChanges" },
                  ),
                }, "stub fix suggestion"),
                h("button", {
                  type: "button",
                  onClick: () => emit(
                    "apply-lilia-provider-settings",
                    { type: "runtime_settings", action: "diagnose" },
                    { provider: { codex: {} } },
                  ),
                }, "stub provider settings"),
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
      { type: "uncommittedChanges" },
    ]);
  });

  it("forwards provider runtime settings events from the composer", async () => {
    const view = renderSurface();

    await fireEvent.click(view.getByRole("button", { name: "stub provider settings" }));

    expect(view.emitted("apply-lilia-provider-settings")?.[0]).toEqual([
      { type: "runtime_settings", action: "diagnose" },
      { provider: { codex: {} } },
    ]);
  });
});
