import { computed, ref } from "vue";

export interface EnvDraftRow {
  key: string;
  value: string;
  originalKey: string | null;
}

export interface EditableMcpServer {
  name: string;
  command: string;
  args: string[];
  envKeys: string[];
}

interface McpServerInput {
  name: string;
  command: string;
  args: string[];
  env?: Record<string, string>;
  removeEnvKeys?: string[];
}

export function useMcpServerEditor<TServer extends EditableMcpServer>({
  label,
  refresh,
  createServer,
  updateServer,
}: {
  label: string;
  refresh: () => Promise<void>;
  createServer: (input: McpServerInput) => Promise<TServer>;
  updateServer: (name: string, input: McpServerInput) => Promise<TServer>;
}) {
  const showMcpEditor = ref(false);
  const editingMcp = ref<TServer | null>(null);
  const mcpName = ref("");
  const mcpCommand = ref("");
  const mcpArgsText = ref("");
  const mcpEnvRows = ref<EnvDraftRow[]>([]);
  const mcpSaving = ref(false);
  const mcpError = ref<string | null>(null);

  const mcpEditorTitle = computed(() =>
    editingMcp.value ? `编辑 ${label}：${editingMcp.value.name}` : `新增 ${label}`,
  );

  function resetMcpEditor(server: TServer | null) {
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

  function openEditMcp(server: TServer) {
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
    const preservedOriginalKeys = new Set<string>(
      mcpEnvRows.value.flatMap((row) =>
        row.originalKey && row.key.trim() === row.originalKey ? [row.originalKey] : [],
      ),
    );
    const removeEnvKeys = editingMcp.value
      ? editingMcp.value.envKeys.filter((key: string) => !preservedOriginalKeys.has(key))
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
        await updateServer(editingMcp.value.name, input);
      } else {
        await createServer(input);
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
