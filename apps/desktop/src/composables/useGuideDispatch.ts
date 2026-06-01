import {
  createTodo,
  listTodos,
  updateTodo,
  type TaskTodo,
  type TaskTodoPriority,
} from "../services/todos";
import type { ChatAttachment } from "@lilia/contracts";

export type GuideDispatchWindow = "tool" | "user" | "idle";

export interface GuideDispatchOptions {
  taskId: () => string;
  ensureDispatchReady: () => Promise<void>;
  ensureReady: (content: string, attachments: ChatAttachment[]) => Promise<void>;
  sendAgentMessage: (content: string, attachments: ChatAttachment[], guideId?: string) => Promise<void>;
  hasPendingAgentAction: () => boolean;
  isTurnRunning: () => boolean;
  clearAttachments: () => void;
  reportError: (message: string) => void;
}

const LILIA_GUIDE_PREFIX = "[Lilia 引导]";
const guidePriorityOrder: TaskTodoPriority[] = ["high", "normal", "low"];

function priorityLabel(priority: TaskTodoPriority): string {
  if (priority === "high") return "高";
  if (priority === "low") return "低";
  return "中";
}

function guideTextForComposer(
  content: string,
  outgoingAttachments: ChatAttachment[],
): string {
  const text = content.trim();
  if (outgoingAttachments.length === 0) return text;
  const attachmentLines = outgoingAttachments.map((attachment, index) =>
    `${index + 1}. ${attachment.name}: ${attachment.path}`
  );
  return [
    text || "请参考以下本地路径继续处理。",
    "",
    "附加路径：",
    ...attachmentLines,
  ].join("\n");
}

function guideMessage(todo: TaskTodo): string {
  return [
    LILIA_GUIDE_PREFIX,
    `优先级：${priorityLabel(todo.priority)}`,
    "",
    todo.text,
  ].join("\n");
}

function allowedPriorities(windowKind: GuideDispatchWindow): Set<TaskTodoPriority> {
  if (windowKind === "tool") return new Set<TaskTodoPriority>(["high"]);
  if (windowKind === "user") return new Set<TaskTodoPriority>(["normal"]);
  return new Set<TaskTodoPriority>(guidePriorityOrder);
}

export function useGuideDispatch(options: GuideDispatchOptions) {
  const dispatchingGuideIds = new Set<string>();
  let autoGuideDispatching = false;
  let pendingGuideWindow: GuideDispatchWindow | null = null;

  async function selectPendingGuide(windowKind: GuideDispatchWindow): Promise<TaskTodo | null> {
    const allowed = allowedPriorities(windowKind);
    const candidates = (await listTodos(options.taskId()))
      .filter((todo) =>
        todo.source === "lilia" &&
        todo.guideStatus === "pending" &&
        !dispatchingGuideIds.has(todo.id) &&
        allowed.has(todo.priority)
      );
    return candidates
      .sort((a, b) =>
        guidePriorityOrder.indexOf(a.priority) - guidePriorityOrder.indexOf(b.priority) ||
        a.order - b.order ||
        a.createdAt - b.createdAt
      )[0] ?? null;
  }

  async function dispatchGuide(todo: TaskTodo) {
    if (todo.source !== "lilia" || dispatchingGuideIds.has(todo.id)) return;
    dispatchingGuideIds.add(todo.id);
    try {
      await options.sendAgentMessage(guideMessage(todo), [], todo.id);
    } catch (err) {
      await updateTodo(todo.id, { guideStatus: "pending" }).catch(() => undefined);
      options.reportError(`插入引导失败：${String(err)}`);
    } finally {
      dispatchingGuideIds.delete(todo.id);
    }
  }

  async function scheduleGuideInsertion(windowKind: GuideDispatchWindow) {
    if (autoGuideDispatching) {
      pendingGuideWindow = windowKind;
      return;
    }
    autoGuideDispatching = true;
    try {
      const guide = await selectPendingGuide(windowKind);
      if (guide) await dispatchGuide(guide);
    } catch (err) {
      options.reportError(`调度引导失败：${String(err)}`);
    } finally {
      autoGuideDispatching = false;
      const nextWindow = pendingGuideWindow;
      pendingGuideWindow = null;
      if (nextWindow) void scheduleGuideInsertion(nextWindow);
    }
  }

  async function createGuideFromComposer(
    content: string,
    outgoingAttachments: ChatAttachment[] = [],
  ) {
    if (!content.trim() && outgoingAttachments.length === 0) return;
    try {
      await options.ensureDispatchReady();
      const guideText = guideTextForComposer(content, outgoingAttachments);
      await options.ensureReady(guideText, []);
      await createTodo(options.taskId(), guideText, "normal");
      options.clearAttachments();
      if (options.hasPendingAgentAction()) {
        void scheduleGuideInsertion("user");
      } else if (!options.isTurnRunning()) {
        void scheduleGuideInsertion("idle");
      }
    } catch (err) {
      options.reportError(`创建引导失败：${String(err)}`);
    }
  }

  return {
    createGuideFromComposer,
    dispatchGuide,
    scheduleGuideInsertion,
  };
}
