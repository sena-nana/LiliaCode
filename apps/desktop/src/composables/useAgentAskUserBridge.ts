import { askUser } from "./useAskUser";
import {
  onAskUserRequest,
  respondAskUser,
  type AgentAskUserRequest,
} from "../services/chat";
import type { AskUserResult } from "@lilia/contracts";

let installed = false;
let unlisten: (() => void) | null = null;

async function answer(req: AgentAskUserRequest) {
  let result: AskUserResult;
  try {
    result = await askUser(req.spec);
  } catch {
    result = { answers: {}, cancelled: true };
  }
  try {
    await respondAskUser(req.taskId, req.requestId, result);
  } catch {
    // runner 可能已经随 turn 结束退出；此时回答无法再写回，忽略即可。
  }
}

/** 在 App 启动时安装一次，把 backend 的用户提问接到全局 AskUser 浮层。 */
export async function installAgentAskUserBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlisten = await onAskUserRequest((req) => {
    void answer(req);
  });
  return () => {
    unlisten?.();
    unlisten = null;
    installed = false;
  };
}
