import { describe, expect, it, vi } from "vitest";
import {
  emitTauriEvent,
  mockInvoke,
} from "./tauriMock";
import {
  getAgentInteractionSettings,
  onAgentInteractionRequest,
  onAskUserRequest,
  respondAgentInteraction,
  respondAskUser,
  setAgentInteractionSettings,
  type AgentInteractionRequest,
  type AgentAskUserRequest,
} from "../src/services/chat";

describe("chat AskUser bridge service", () => {
  it("订阅 Claude 用户提问事件并把 AskUserResult 写回 runner", async () => {
    const handler = vi.fn<(event: AgentAskUserRequest) => void>();
    await onAskUserRequest(handler);

    emitTauriEvent("chat:ask-user-request", {
      taskId: "task-1",
      turnId: "turn-1",
      backend: "claude",
      requestId: "ask-1",
      spec: {
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
      },
    });

    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({
        taskId: "task-1",
        requestId: "ask-1",
        spec: expect.objectContaining({ source: "Claude" }),
      }),
    );

    await respondAskUser("task-1", "ask-1", {
      cancelled: false,
      answers: {
        "q-1": { questionId: "q-1", value: "o-2" },
      },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_ask_user", {
      taskId: "task-1",
      requestId: "ask-1",
      result: {
        cancelled: false,
        answers: {
          "q-1": { questionId: "q-1", value: "o-2" },
        },
      },
    }, undefined);
  });

  it("订阅统一 Agent interaction 并把响应写回 runner", async () => {
    const handler = vi.fn<(event: AgentInteractionRequest) => void>();
    await onAgentInteractionRequest(handler);

    emitTauriEvent("chat:agent-interaction-request", {
      taskId: "task-1",
      turnId: "turn-1",
      backend: "codex",
      requestId: "ask-1",
      kind: "plan_approval",
      payload: {
        title: "确认 Codex 计划",
        intent: "plan_approval",
        questions: [
          {
            id: "approve-plan",
            question: "",
            mode: "confirm",
          },
        ],
      },
    });

    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({
        backend: "codex",
        kind: "plan_approval",
        payload: expect.objectContaining({ title: "确认 Codex 计划" }),
      }),
    );

    await respondAgentInteraction({
      taskId: "task-1",
      requestId: "ask-1",
      kind: "plan_approval",
      result: {
        cancelled: false,
        answers: {
          "approve-plan": { questionId: "approve-plan", value: "yes" },
        },
      },
    });

    expect(mockInvoke).toHaveBeenCalledWith("chat_respond_agent_interaction", {
      taskId: "task-1",
      requestId: "ask-1",
      kind: "plan_approval",
      result: {
        cancelled: false,
        answers: {
          "approve-plan": { questionId: "approve-plan", value: "yes" },
        },
      },
    }, undefined);
  });

  it("Agent 交互设置默认关闭，并能保存非打断模式", async () => {
    await expect(getAgentInteractionSettings()).resolves.toEqual({
      nonInterruptMode: false,
      debug: false,
    });

    await setAgentInteractionSettings({ nonInterruptMode: true });

    await expect(getAgentInteractionSettings()).resolves.toEqual({
      nonInterruptMode: true,
      debug: false,
    });
  });
});
