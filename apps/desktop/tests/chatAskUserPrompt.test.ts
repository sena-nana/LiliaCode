import { fireEvent, render, waitFor } from "@testing-library/vue";
import { createMemoryHistory } from "vue-router";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import TaskDetail from "../src/pages/TaskDetail.vue";
import { installAgentAskUserBridge } from "../src/composables/useAgentAskUserBridge";
import { resolveAskUser, useAskUser } from "../src/composables/useAskUser";
import { createLiliaRouter } from "../src/router";
import { projectsReady } from "../src/data/projects";
import { allTasksReady } from "../src/data/tasks";
import {
  emitTauriEvent,
  mockInvoke,
} from "./tauriMock";

const askUserSpec = {
  title: "Claude 想确认一下",
  source: "Claude",
  questions: [
    {
      id: "q-1",
      header: "方案",
      question: "选哪个方案？",
      mode: "single",
      options: [
        { id: "o-1", label: "A" },
        { id: "o-2", label: "B" },
      ],
    },
  ],
};

async function renderTaskDetail(taskId = "t-002") {
  const router = createLiliaRouter(createMemoryHistory());
  await router.push(`/projects/lilia/tasks/${taskId}`);
  await router.isReady();

  return render(TaskDetail, {
    props: {
      projectId: "lilia",
      taskId,
    },
    global: {
      plugins: [router],
    },
  });
}

function emitAskUserRequest(taskId: string) {
  emitTauriEvent("chat:ask-user-request", {
    taskId,
    turnId: "turn-ask",
    backend: "claude",
    requestId: `ask-${taskId}`,
    spec: askUserSpec,
  });
}

function emitSnakeCaseAskUserRequest(taskId: string) {
  emitTauriEvent("chat:ask-user-request", {
    task_id: taskId,
    turn_id: "turn-ask",
    backend: "claude",
    request_id: `ask-${taskId}`,
    spec: askUserSpec,
  });
}

async function expectAskUserResponse(taskId: string) {
  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
      taskId,
      requestId: `ask-${taskId}`,
      result: {
        cancelled: false,
        answers: {
          "q-1": { questionId: "q-1", value: "o-2" },
        },
      },
    }, undefined);
  });
}

let unlistenAskUser: (() => void) | null = null;

describe("chat AskUser prompt", () => {
  beforeEach(async () => {
    await Promise.all([projectsReady, allTasksReady]);
    unlistenAskUser = await installAgentAskUserBridge();
  });

  afterEach(async () => {
    const { state } = useAskUser();
    while (state.current || state.queue.length) {
      resolveAskUser({ answers: {}, cancelled: true });
    }
    await Promise.resolve();

    unlistenAskUser?.();
    unlistenAskUser = null;
  });

  it("把当前 task 的 Agent 提问显示在 composer 上方，并保留回答回写", async () => {
    const view = await renderTaskDetail();

    emitAskUserRequest("t-002");

    const prompt = await view.findByRole("region", { name: "Claude 想确认一下" });
    expect(prompt).toHaveClass("ask-user");
    expect(document.body.querySelector(".search-palette.ask-user")).toBeNull();

    const controls = view.container.querySelector(".chat-controls");
    const composer = controls?.querySelector(".chat-composer");
    expect(controls).toContainElement(prompt);
    expect(composer).not.toBeNull();
    expect(prompt.compareDocumentPosition(composer!) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();

    await fireEvent.click(view.getByRole("radio", { name: "B" }));

    await expectAskUserResponse("t-002");
  });

  it("不会在当前对话显示其他 task 的 Agent 提问", async () => {
    const view = await renderTaskDetail("t-002");

    emitAskUserRequest("t-003");

    await waitFor(() => {
      expect(useAskUser().state.current?.taskId).toBe("t-003");
    });
    expect(view.queryByRole("region", { name: "Claude 想确认一下" })).toBeNull();
    expect(view.container.querySelector(".chat-controls .ask-user")).toBeNull();
  });

  it("兼容 Tauri 事件里 snake_case 的 task 和 request 字段", async () => {
    const view = await renderTaskDetail();

    emitSnakeCaseAskUserRequest("t-002");

    await view.findByRole("region", { name: "Claude 想确认一下" });
    await fireEvent.click(view.getByRole("radio", { name: "B" }));

    await expectAskUserResponse("t-002");
  });
});
