import { invoke as rawInvoke, convertFileSrc } from "@tauri-apps/api/core";
import {
  recordAgentDebugInvokeEnd,
  recordAgentDebugInvokeStart,
} from "../agentDebug/harness";

export { convertFileSrc };

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const trace = recordAgentDebugInvokeStart(cmd, args ?? {});
  try {
    const result = await rawInvoke<T>(cmd, args);
    recordAgentDebugInvokeEnd(trace, "success", result);
    return result;
  } catch (err) {
    recordAgentDebugInvokeEnd(trace, "error", err);
    throw err;
  }
}
