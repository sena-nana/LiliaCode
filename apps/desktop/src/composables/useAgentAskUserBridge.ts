import { askUserForTask } from "./useAskUser";
import {
  onAskUserRequest,
  respondAskUser,
  type AgentAskUserRequest,
} from "../services/chat";
import type { AskUserResult, AskUserSpec } from "@lilia/contracts";

let installed = false;
let unlisten: (() => void) | null = null;

function stringField(row: Record<string, unknown>, camel: string, snake: string): string {
  const value = row[camel] ?? row[snake];
  return typeof value === "string" ? value : "";
}

function specField(row: Record<string, unknown>): AskUserSpec | null {
  const value = row.spec;
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as AskUserSpec;
}

async function answer(req: AgentAskUserRequest) {
  const row = req as unknown as Record<string, unknown>;
  const taskId = stringField(row, "taskId", "task_id");
  const turnId = stringField(row, "turnId", "turn_id");
  const requestId = stringField(row, "requestId", "request_id");
  const spec = specField(row);
  if (!taskId || !requestId || !spec) return;

  let result: AskUserResult;
  try {
    result = await askUserForTask(taskId, spec, turnId || null, requestId);
  } catch {
    result = { answers: {}, cancelled: true };
  }
  try {
    await respondAskUser(taskId, requestId, result);
  } catch {
    // runner 可能已经随 turn 结束退出；此时回答无法再写回，忽略即可。
  }
}

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
