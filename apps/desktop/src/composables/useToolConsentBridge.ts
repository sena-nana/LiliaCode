/**
 * Tool-consent bridge：把 runner 通过 `chat:tool-consent-request` 推过来的工具
 * 授权请求收进一个按 taskId 索引的 reactive map，供 ChatComposer 在
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
  onToolConsentRequest,
  respondToolConsent,
  type ToolConsentDecision,
  type ToolConsentRequest,
} from "../services/chat";

const pending = reactive<Record<string, ToolConsentRequest>>({});
const localResolvers = new Map<
  string,
  (decision: ToolConsentDecision, message?: string) => void
>();
let installed = false;
let unlisten: (() => void) | null = null;

type TaskIdSource = string | (() => string);

function readTaskId(source: TaskIdSource): string {
  return typeof source === "function" ? source() : source;
}

/** 在 App 启动时调用一次。返回 unlisten；重复 install 时返回 noop。 */
export async function installToolConsentBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlisten = await onToolConsentRequest((req) => {
    pending[req.taskId] = req;
    localResolvers.delete(req.requestId);
  });
  return () => {
    unlisten?.();
    unlisten = null;
    installed = false;
  };
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
): Promise<{ decision: ToolConsentDecision; message?: string }> {
  pending[request.taskId] = request;
  return new Promise((resolve) => {
    localResolvers.set(request.requestId, (decision, message) => {
      resolve({ decision, message });
    });
  });
}

/** 提交决策：写回 runner 后立即从 pending 移除，让 inline 卡片淡出。 */
export async function respondConsent(
  taskId: string,
  requestId: string,
  decision: ToolConsentDecision,
  message?: string,
): Promise<void> {
  // 先乐观移除——用户已经做了选择，UI 不应再"卡"在原卡片上。
  // 即便 invoke 失败，也只是 runner 没收到决策，下次会用同 id 再发一次。
  if (pending[taskId]?.requestId === requestId) {
    delete pending[taskId];
  }
  const localResolve = localResolvers.get(requestId);
  if (localResolve) {
    localResolvers.delete(requestId);
    localResolve(decision, message);
    return;
  }
  await respondToolConsent(taskId, requestId, decision, message);
}
