import { askUserForTask } from "./useAskUser";
import { respondAgentInteraction } from "../services/chat";
import type { AskUserResult, AskUserSpec, ChatBackendKind } from "@lilia/contracts";

export interface AgentAskUserRequest {
  taskId: string;
  turnId: string;
  backend: ChatBackendKind;
  requestId: string;
  spec: AskUserSpec;
}

export async function handleAgentAskUserRequest(
  req: AgentAskUserRequest,
  kind: "ask_user" | "plan_approval" = req.spec.intent === "plan_approval" ? "plan_approval" : "ask_user",
) {
  let result: AskUserResult;
  try {
    result = await askUserForTask(req.taskId, req.spec, req.turnId || null, req.requestId);
  } catch {
    result = { answers: {}, cancelled: true };
  }
  try {
    await respondAgentInteraction({
      taskId: req.taskId,
      requestId: req.requestId,
      kind,
      result,
    });
  } catch {
    // runner 可能已经随 turn 结束退出；此时回答无法再写回，忽略即可。
  }
}
