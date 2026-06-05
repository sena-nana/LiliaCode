import { describe, expect, it, vi } from "vitest";
import {
  emitTauriEvent,
  mockInvoke,
} from "./tauriMock";
import {
  getAgentInteractionSettings,
  onAgentInteractionRequest,
  respondAgentInteraction,
  setAgentInteractionSettings,
  type AgentInteractionRequest,
} from "../src/services/chat";

describe("chat AskUser bridge service", () => {
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
      codexProfile: {
        profile: "default",
        model: null,
        reasoningEffort: null,
        runtimeWorkspaceRoots: [],
        permissions: { profile: "default" },
      },
    });

    await setAgentInteractionSettings({ nonInterruptMode: true });

    await expect(getAgentInteractionSettings()).resolves.toEqual({
      nonInterruptMode: true,
      debug: false,
      codexProfile: {
        profile: "default",
        model: null,
        reasoningEffort: null,
        runtimeWorkspaceRoots: [],
        permissions: { profile: "default" },
      },
    });
  });
});
