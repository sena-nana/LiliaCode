import { describe, expect, it } from "vitest";
import {
  chatAttachmentsToPayload,
  createErrorTimelineEvent,
  createMessageTimelineEvent,
  mergeLoadedTimelineEvents,
  mergeTimelineEvents,
  retryContextForTimelineEvent,
  upsertTimelineEventsById,
  type AgentTimelineEvent,
  type ChatAttachment,
  type ChatConversationReference,
} from "@lilia/contracts";
import { timelineEventFixture } from "./timelineTestHelpers";

function event(overrides: Partial<AgentTimelineEvent>): AgentTimelineEvent {
  return timelineEventFixture(overrides, {
    id: "event",
    taskId: "t-1",
    turnId: null,
    backend: "claude",
    kind: "info",
  });
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

const conversationReference: ChatConversationReference = {
  taskId: "task-old",
  title: "旧对话",
  route: "/projects/p-1/tasks/task-old",
  projectId: "p-1",
  projectName: "主项目",
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

  it("批量 upsert timeline 事件时只保留同 id 最新快照并稳定排序", () => {
    const merged = upsertTimelineEventsById(
      [
        event({ id: "a", turnSeq: 2, intraTurnOrder: 0, summary: "旧" }),
        event({ id: "b", turnSeq: 1, intraTurnOrder: 1 }),
      ],
      [
        event({ id: "a", turnSeq: 2, intraTurnOrder: 0, summary: "新" }),
        event({ id: "c", turnSeq: 1, intraTurnOrder: 0 }),
      ],
    );

    expect(merged.map((item) => item.id)).toEqual(["c", "b", "a"]);
    expect(merged.find((item) => item.id === "a")?.summary).toBe("新");
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

  it("完整加载合并时保留显式指定的当前同 id 事件", () => {
    const loaded = event({ id: "tool-1", status: "running", summary: "旧快照", updatedAt: 10 });
    const live = event({ id: "tool-1", status: "requires_action", summary: "实时更新", updatedAt: 20 });

    const merged = mergeLoadedTimelineEvents([loaded], [live], new Set(["tool-1"]));

    expect(merged).toHaveLength(1);
    expect(merged[0]?.status).toBe("requires_action");
    expect(merged[0]?.summary).toBe("实时更新");
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
        conversationReferences: [conversationReference],
      },
    });

    expect(retryContextForTimelineEvent(error, [])).toEqual({
      content: "重试这句",
      attachments: [expect.objectContaining({ id: "a-1" })],
      conversationReferences: [conversationReference],
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
        conversationReferences: [{
          ...conversationReference,
          projectId: null,
          projectName: null,
        }],
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
      conversationReferences: [expect.objectContaining({
        taskId: conversationReference.taskId,
        title: conversationReference.title,
        route: conversationReference.route,
      })],
    });
  });

  it("消息 payload 和重试恢复共享同一份对话引用序列化结果", () => {
    const message = createMessageTimelineEvent({
      id: "message-1",
      taskId: "t-1",
      backend: "claude",
      content: "引用旧对话",
      attachments: [],
      conversationReferences: [conversationReference],
      createdAt: 10,
    });
    const error = createErrorTimelineEvent({
      id: "error-1",
      taskId: "t-1",
      backend: "claude",
      message: "发送失败",
      createdAt: 20,
      retryContext: {
        content: "引用旧对话",
        attachments: [],
        conversationReferences: [conversationReference],
      },
    });

    expect((message.payload as Record<string, unknown>).conversationReferences).toEqual([
      {
        taskId: "task-old",
        title: "旧对话",
        route: "/projects/p-1/tasks/task-old",
        projectId: "p-1",
        projectName: "主项目",
      },
    ]);
    expect(retryContextForTimelineEvent(error, [message])).toEqual({
      content: "引用旧对话",
      attachments: [],
      conversationReferences: [conversationReference],
    });
  });

  it("附件 payload 保留目录摘要并补齐 nullable 字段", () => {
    const payload = chatAttachmentsToPayload([
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
