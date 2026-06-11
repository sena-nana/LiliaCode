/**
 * Tool-consent bridge：把统一 Agent interaction 推过来的工具授权请求
 * 收进一个按 taskId 索引的 reactive map，供 ChatComposer 在
 * 各自任务的输入框内部 inline 渲染。
 *
 * 这里只做"收事件 + 写回决策"两件事，不接 askUser/弹窗；inline 卡片决定一切
 * 视觉与交互。
 *
 * 同一 task 同一时刻只可能有一个待决策项（runner 端的 canUseTool 是串行的），
 * 因此 map 用 taskId → ToolConsentRequest 即可；多个任务并发时各自独立。
 */

import { reactive, computed, type ComputedRef } from "vue";
import {
  respondAgentInteraction,
  type ToolConsentDecision,
  type ToolConsentRequest,
  type ToolConsentUpdatedInput,
} from "../services/chat";
import {
  clearConversationRequiresAction,
  markConversationRequiresAction,
} from "./useConversationActivity";

const pending = reactive<Record<string, ToolConsentRequest>>({});
const localResolvers = new Map<
  string,
  (
    decision: ToolConsentDecision,
    message?: string,
    updatedInput?: ToolConsentUpdatedInput,
  ) => void
>();

type TaskIdSource = string | (() => string);

function readTaskId(source: TaskIdSource): string {
  return typeof source === "function" ? source() : source;
}

export interface ClearToolConsentForTaskOptions {
  turnId?: string | null;
  keepRequestIds?: Set<string>;
}

/** 给某个 task 订阅当前待决策项（没有就是 null）。 */
export function useToolConsentForTask(
  taskId: TaskIdSource,
): ComputedRef<ToolConsentRequest | null> {
  return computed(() => pending[readTaskId(taskId)] ?? null);
}

export function usePendingToolConsentsForTask(
  taskId: TaskIdSource,
): ComputedRef<ToolConsentRequest[]> {
  return computed(() => {
    const request = pending[readTaskId(taskId)];
    return request ? [request] : [];
  });
}

export function requestLocalToolConsent(
  request: ToolConsentRequest,
): Promise<{
  decision: ToolConsentDecision;
  message?: string;
  updatedInput?: ToolConsentUpdatedInput;
}> {
  pending[request.taskId] = request;
  markConversationRequiresAction(request.taskId, request.requestId);
  return new Promise((resolve) => {
    localResolvers.set(request.requestId, (decision, message, updatedInput) => {
      resolve({ decision, message, updatedInput });
    });
  });
}

export function handleToolConsentRequest(
  request: ToolConsentRequest,
) {
  pending[request.taskId] = request;
  markConversationRequiresAction(request.taskId, request.requestId);
  localResolvers.delete(request.requestId);
}

export function hydrateToolConsentRequest(request: ToolConsentRequest) {
  pending[request.taskId] = request;
  markConversationRequiresAction(request.taskId, request.requestId);
  localResolvers.delete(request.requestId);
}

export function clearToolConsentForTask(
  taskId: string,
  options: ClearToolConsentForTaskOptions = {},
) {
  const request = pending[taskId];
  if (!request) return;
  if (options.turnId !== undefined && request.turnId !== options.turnId) return;
  if (options.keepRequestIds?.has(request.requestId)) return;
  delete pending[taskId];
  localResolvers.delete(request.requestId);
  clearConversationRequiresAction(taskId, request.requestId);
}

/** 提交决策：写回 runner 后立即从 pending 移除，让 inline 卡片淡出。 */
export async function respondConsent(
  taskId: string,
  requestId: string,
  decision: ToolConsentDecision,
  message?: string,
  updatedInput?: ToolConsentUpdatedInput,
  codexDecision?: string,
): Promise<void> {
  const localResolve = localResolvers.get(requestId);
  if (localResolve) {
    if (pending[taskId]?.requestId === requestId) {
      delete pending[taskId];
    }
    clearConversationRequiresAction(taskId, requestId);
    localResolvers.delete(requestId);
    localResolve(decision, message, updatedInput);
    return;
  }
  await respondAgentInteraction({
    taskId,
    requestId,
    kind: "tool_consent",
    result: {
      taskId,
      requestId,
      decision,
      message: message ?? null,
      ...(updatedInput ? { updatedInput } : {}),
      ...(codexDecision ? { codexDecision } : {}),
    },
  });
  if (pending[taskId]?.requestId === requestId) {
    delete pending[taskId];
  }
  clearConversationRequiresAction(taskId, requestId);
}
