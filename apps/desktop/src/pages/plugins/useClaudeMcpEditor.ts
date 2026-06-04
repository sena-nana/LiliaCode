import { computed, ref } from "vue";
import {
  createClaudeMcpServer,
  updateClaudeMcpServer,
  type ClaudeMcpServer,
} from "../../services/plugins";

export interface EnvDraftRow {
  key: string;
  value: string;
  originalKey: string | null;
}

export function useClaudeMcpEditor({ refresh }: { refresh: () => Promise<void> }) {
  const showMcpEditor = ref(false);
  const editingMcp = ref<ClaudeMcpServer | null>(null);
  const mcpName = ref("");
  const mcpCommand = ref("");
  const mcpArgsText = ref("");
  const mcpEnvRows = ref<EnvDraftRow[]>([]);
  const mcpSaving = ref(false);
  const mcpError = ref<string | null>(null);

  const mcpEditorTitle = computed(() =>
    editingMcp.value ? `编辑 Claude MCP：${editingMcp.value.name}` : "新增 Claude MCP",
  );

  function resetMcpEditor(server: ClaudeMcpServer | null) {
    editingMcp.value = server;
    mcpName.value = server?.name ?? "";
    mcpCommand.value = server?.command ?? "";
    mcpArgsText.value = server?.args.join("\n") ?? "";
    mcpEnvRows.value = server?.envKeys.length
      ? server.envKeys.map((key) => ({ key, value: "", originalKey: key }))
      : [{ key: "", value: "", originalKey: null }];
    mcpError.value = null;
  }

  function openCreateMcp() {
    resetMcpEditor(null);
    showMcpEditor.value = true;
  }

  function openEditMcp(server: ClaudeMcpServer) {
    resetMcpEditor(server);
    showMcpEditor.value = true;
  }

  function addMcpEnvRow() {
    mcpEnvRows.value.push({ key: "", value: "", originalKey: null });
  }

  function removeMcpEnvRow(index: number) {
    mcpEnvRows.value.splice(index, 1);
    if (mcpEnvRows.value.length === 0) {
      mcpEnvRows.value.push({ key: "", value: "", originalKey: null });
    }
  }

  function buildMcpEnvPatch() {
    const env = Object.fromEntries(
      mcpEnvRows.value
        .map((row) => [row.key.trim(), row.value] as const)
        .filter(([key, value]) => key && value),
    );
    const preservedOriginalKeys = new Set(
      mcpEnvRows.value.flatMap((row) =>
        row.originalKey && row.key.trim() === row.originalKey ? [row.originalKey] : [],
      ),
    );
    const removeEnvKeys = editingMcp.value
      ? editingMcp.value.envKeys.filter((key) => !preservedOriginalKeys.has(key))
      : [];
    return { env, removeEnvKeys };
  }

  async function saveMcpServer() {
    if (mcpSaving.value) return;
    mcpError.value = null;
    mcpSaving.value = true;
    try {
      const { env, removeEnvKeys } = buildMcpEnvPatch();
      const input = {
        name: mcpName.value,
        command: mcpCommand.value,
        args: mcpArgsText.value
          .split(/\r?\n/)
          .map((arg) => arg.trim())
          .filter(Boolean),
        ...(Object.keys(env).length > 0 ? { env } : {}),
        ...(removeEnvKeys.length > 0 ? { removeEnvKeys } : {}),
      };
      if (editingMcp.value) {
        await updateClaudeMcpServer(editingMcp.value.name, input);
      } else {
        await createClaudeMcpServer(input);
      }
      showMcpEditor.value = false;
      await refresh();
    } catch (err) {
      mcpError.value = String(err);
    } finally {
      mcpSaving.value = false;
    }
  }

  return {
    showMcpEditor,
    editingMcp,
    mcpName,
    mcpCommand,
    mcpArgsText,
    mcpEnvRows,
    mcpSaving,
    mcpError,
    mcpEditorTitle,
    openCreateMcp,
    openEditMcp,
    addMcpEnvRow,
    removeMcpEnvRow,
    saveMcpServer,
  };
}
