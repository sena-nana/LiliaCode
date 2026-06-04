import { askUserForTask } from "./useAskUser";
import {
  onAskUserRequest,
  respondAgentInteraction,
  respondAskUser,
  type AgentAskUserRequest,
} from "../services/chat";
import type { AskUserResult } from "@lilia/contracts";

let installed = false;
let unlisten: (() => void) | null = null;

export async function handleAgentAskUserRequest(
  req: AgentAskUserRequest,
  options: { unified?: boolean; kind?: "ask_user" | "plan_approval" } = {},
) {
  let result: AskUserResult;
  try {
    result = await askUserForTask(req.taskId, req.spec, req.turnId || null, req.requestId);
  } catch {
    result = { answers: {}, cancelled: true };
  }
  try {
    if (options.unified) {
      await respondAgentInteraction({
        taskId: req.taskId,
        requestId: req.requestId,
        kind: options.kind ?? (req.spec.intent === "plan_approval" ? "plan_approval" : "ask_user"),
        result,
      });
      return;
    }
    await respondAskUser(req.taskId, req.requestId, result);
  } catch {
    // runner 可能已经随 turn 结束退出；此时回答无法再写回，忽略即可。
  }
}

export async function installAgentAskUserBridge(): Promise<() => void> {
  if (installed) return () => {};
  installed = true;
  unlisten = await onAskUserRequest((req) => {
    void handleAgentAskUserRequest(req);
  });
  return () => {
    unlisten?.();
    unlisten = null;
    installed = false;
  };
}
