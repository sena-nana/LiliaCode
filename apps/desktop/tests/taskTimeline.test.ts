import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent, ChatAttachment } from "@lilia/contracts";
import {
  attachmentsToTimelinePayload,
  createErrorTimelineEvent,
  createMessageTimelineEvent,
  mergeLoadedTimelineEvents,
  mergeTimelineEvents,
  retryContextForTimelineEvent,
} from "../src/pages/taskDetail/useTaskTimeline";

function event(overrides: Partial<AgentTimelineEvent>): AgentTimelineEvent {
  return {
    id: "event",
    taskId: "t-1",
    turnId: null,
    backend: "claude",
    kind: "info",
    status: "success",
    title: "事件",
    summary: null,
    payload: null,
    createdAt: 1,
    updatedAt: 1,
    turnSeq: 1,
    intraTurnOrder: 1,
    ...overrides,
  };
}

const attachment: ChatAttachment = {
  id: "a-1",
  name: "README.md",
  path: "C:\\Files\\workspace\\Lilia\\README.md",
  kind: "file",
  size: 120,
  exists: true,
  mime: "text/markdown",
};

describe("task detail timeline helpers", () => {
  it("合并 timeline 时按 turn 和事件顺序稳定排序", () => {
    const merged = mergeTimelineEvents(
      [
        event({ id: "b", turnSeq: 2, intraTurnOrder: 1, createdAt: 20 }),
        event({ id: "a", turnSeq: 1, intraTurnOrder: 2, createdAt: 10 }),
      ],
      [
        event({ id: "c", turnSeq: 1, intraTurnOrder: 1, createdAt: 30 }),
      ],
    );

    expect(merged.map((item) => item.id)).toEqual(["c", "a", "b"]);
  });

  it("加载持久化消息后去掉等价的乐观用户消息", () => {
    const optimistic = createMessageTimelineEvent({
      id: "pending-1",
      taskId: "t-1",
      backend: "claude",
      content: "整理 README",
      attachments: [attachment],
      createdAt: 10,
      queued: true,
    });
    const persisted = createMessageTimelineEvent({
      id: "db-1",
      taskId: "t-1",
      backend: "claude",
      content: "整理 README",
      attachments: [attachment],
      createdAt: 20,
    });

    const merged = mergeLoadedTimelineEvents([persisted], [optimistic]);

    expect(merged.map((item) => item.id)).toEqual(["db-1"]);
  });

  it("从错误事件内嵌 payload 读取重试上下文", () => {
    const error = createErrorTimelineEvent({
      id: "error-1",
      taskId: "t-1",
      backend: "claude",
      message: "发送失败",
      createdAt: 10,
      retryContext: {
        content: "重试这句",
        attachments: [attachment],
      },
    });

    expect(retryContextForTimelineEvent(error, [])).toEqual({
      content: "重试这句",
      attachments: [expect.objectContaining({ id: "a-1" })],
    });
  });

  it("可从同 turn 用户消息回溯错误重试上下文", () => {
    const userMessage = event({
      id: "message-1",
      kind: "message",
      turnId: "turn-1",
      summary: "从 turn 读取",
      payload: {
        role: "user",
        content: "从 turn 读取",
        attachments: [attachment],
      },
    });
    const error = event({
      id: "error-1",
      kind: "error",
      status: "error",
      turnId: "turn-1",
      summary: "失败",
      payload: { message: "失败" },
    });

    expect(retryContextForTimelineEvent(error, [userMessage, error])).toEqual({
      content: "从 turn 读取",
      attachments: [expect.objectContaining({ path: attachment.path })],
    });
  });

  it("附件 payload 保留目录摘要并补齐 nullable 字段", () => {
    const payload = attachmentsToTimelinePayload([
      {
        ...attachment,
        directory: {
          fileCount: 2,
          directoryCount: 1,
          totalSize: 240,
          truncated: false,
          unreadableCount: 0,
        },
      },
    ]);

    expect(payload).toEqual([
      expect.objectContaining({
        exists: true,
        mime: "text/markdown",
        directory: {
          fileCount: 2,
          directoryCount: 1,
          totalSize: 240,
          truncated: false,
          unreadableCount: 0,
        },
      }),
    ]);
  });
});
