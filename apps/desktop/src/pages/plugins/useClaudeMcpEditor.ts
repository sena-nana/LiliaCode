import {
  createClaudeMcpServer,
  updateClaudeMcpServer,
  type ClaudeMcpServer,
} from "../../services/plugins";
import { useMcpServerEditor, type EnvDraftRow } from "./useMcpServerEditor";

export type { EnvDraftRow };

export function useClaudeMcpEditor({ refresh }: { refresh: () => Promise<void> }) {
  return useMcpServerEditor<ClaudeMcpServer>({
    label: "Claude MCP",
    refresh,
    createServer: createClaudeMcpServer,
    updateServer: updateClaudeMcpServer,
  });
}
