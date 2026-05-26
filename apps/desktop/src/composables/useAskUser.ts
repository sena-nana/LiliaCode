/**
 * Agent → 用户「求确认 / 求选择」浮层的全局编排器。
 *
 * 设计要点：
 * - 单例宿主：AskUserHost.vue 在 App 根挂一次，监听这里的 reactive state；
 *   外部任何位置都能 `askUser(spec)` 弹窗并 await 拿到 AskUserResult。
 * - 串行排队：同一时刻只展示一个 spec。后到的 askUser 进队列，前一个结算后才接力，
 *   避免 N 个浮层互相遮挡或竞争焦点。
 * - 多问题步进：单个 spec 内含 N 道题时，UI 保持同一张卡片做 1/N 步进，
 *   组件内部维护当前 index 与回答 map，最终一次性把 AskUserResult resolve 回来。
 *
 * 这里只管「描述当前在问什么 + 收回答」。所有视觉与交互都交给 AskUserDialog.vue。
 */

import { reactive } from "vue";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";

interface PendingAsk {
  spec: AskUserSpec;
  resolve: (result: AskUserResult) => void;
}

interface AskUserState {
  /** 当前正在展示的 spec；null 表示空闲。 */
  current: PendingAsk | null;
  /** 等待中的 spec 队列，FIFO。 */
  queue: PendingAsk[];
}

const state = reactive<AskUserState>({
  current: null,
  queue: [],
});

function pumpNext() {
  if (state.current) return;
  const next = state.queue.shift();
  if (next) state.current = next;
}

/**
 * 弹出一组问题，等用户全部走完后 resolve。
 * - 用户中途关闭：resolve {cancelled: true, answers: 已答的}
 * - 单题 spec：UI 会自动隐藏 1/1 角标，等价于一次性 confirm/select
 */
export function askUser(spec: AskUserSpec): Promise<AskUserResult> {
  return new Promise((resolve) => {
    state.queue.push({ spec, resolve });
    pumpNext();
  });
}

/** AskUserDialog 提交时调用：把结果回给 await 端，并自动接力下一个排队 spec。 */
export function resolveAskUser(result: AskUserResult) {
  const current = state.current;
  if (!current) return;
  state.current = null;
  current.resolve(result);
  pumpNext();
}

export function useAskUser() {
  return { state };
}
