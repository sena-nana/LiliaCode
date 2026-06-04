import {
  createCodexMcpServer,
  updateCodexMcpServer,
  type CodexMcpServer,
} from "../../services/plugins";
import { useMcpServerEditor } from "./useMcpServerEditor";

export function useCodexMcpEditor({ refresh }: { refresh: () => Promise<void> }) {
  return useMcpServerEditor<CodexMcpServer>({
    label: "Codex MCP",
    refresh,
    createServer: createCodexMcpServer,
    updateServer: updateCodexMcpServer,
  });
}
