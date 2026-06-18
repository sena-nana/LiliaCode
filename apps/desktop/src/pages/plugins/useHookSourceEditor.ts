import { computed, ref } from "vue";
import {
  updateHookSource,
  type HookDocumentView,
  type HookHandlerUpdateInput,
  type HookSourceSummary,
} from "../../services/plugins";

export interface HookHandlerDraftRow {
  id: string;
  event: string;
  matcher: string;
  type: string;
  command: string;
  commandWindows: string;
  timeoutSeconds: string;
  statusMessage: string;
  groupAdvancedJson: string;
  advancedJson: string;
}

function createDraftRow(): HookHandlerDraftRow {
  return {
    id: "",
    event: "",
    matcher: "",
    type: "command",
    command: "",
    commandWindows: "",
    timeoutSeconds: "",
    statusMessage: "",
    groupAdvancedJson: "",
    advancedJson: "",
  };
}

function draftFromHandler(handler: HookDocumentView["handlers"][number]): HookHandlerDraftRow {
  return {
    id: handler.id,
    event: handler.event,
    matcher: handler.matcher ?? "",
    type: handler.type,
    command: handler.command ?? "",
    commandWindows: handler.commandWindows ?? "",
    timeoutSeconds: handler.timeoutSeconds == null ? "" : String(handler.timeoutSeconds),
    statusMessage: handler.statusMessage ?? "",
    groupAdvancedJson: handler.groupAdvancedJson ?? "",
    advancedJson: handler.advancedJson ?? "",
  };
}

function normalizeOptionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function validateJsonField(value: string, label: string) {
  const trimmed = value.trim();
  if (!trimmed) return;
  const parsed = JSON.parse(trimmed);
  if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
    throw new Error(`${label} 必须是 JSON 对象`);
  }
}

export function useHookSourceEditor({
  refresh,
  onSaved,
}: {
  refresh: () => Promise<void>;
  onSaved?: (document: HookDocumentView) => void;
}) {
  const showHookEditor = ref(false);
  const editingSource = ref<HookSourceSummary | null>(null);
  const hookHandlerRows = ref<HookHandlerDraftRow[]>([]);
  const hookSaving = ref(false);
  const hookError = ref<string | null>(null);

  const hookEditorTitle = computed(() =>
    editingSource.value ? `编辑 Hooks 来源：${editingSource.value.name}` : "编辑 Hooks 来源",
  );

  function openHookEditor(document: HookDocumentView) {
    editingSource.value = document.source;
    hookHandlerRows.value = document.handlers.length
      ? document.handlers.map((handler) => draftFromHandler(handler))
      : [createDraftRow()];
    hookError.value = null;
    showHookEditor.value = true;
  }

  function addHookHandler() {
    hookHandlerRows.value.push(createDraftRow());
  }

  function removeHookHandler(index: number) {
    hookHandlerRows.value.splice(index, 1);
    if (hookHandlerRows.value.length === 0) {
      hookHandlerRows.value.push(createDraftRow());
    }
  }

  function buildHookHandlersInput(): HookHandlerUpdateInput[] {
    return hookHandlerRows.value.flatMap((row) => {
      const hasContent = [
        row.event,
        row.matcher,
        row.command,
        row.commandWindows,
        row.timeoutSeconds,
        row.statusMessage,
        row.groupAdvancedJson,
        row.advancedJson,
      ].some((value) => value.trim());
      if (!hasContent && !row.type.trim()) return [];
      if (!row.event.trim()) {
        throw new Error("每条 hook 都需要 event");
      }
      if (!row.type.trim()) {
        throw new Error("每条 hook 都需要 type");
      }
      validateJsonField(row.groupAdvancedJson, "Group JSON");
      validateJsonField(row.advancedJson, "Handler JSON");
      const timeoutText = row.timeoutSeconds.trim();
      let timeoutSeconds: number | null = null;
      if (timeoutText) {
        const parsed = Number.parseInt(timeoutText, 10);
        if (!Number.isFinite(parsed) || parsed < 0) {
          throw new Error("Timeout 必须是非负整数");
        }
        timeoutSeconds = parsed;
      }
      return [{
        id: row.id || undefined,
        event: row.event.trim(),
        matcher: normalizeOptionalText(row.matcher),
        type: row.type.trim(),
        command: normalizeOptionalText(row.command),
        commandWindows: normalizeOptionalText(row.commandWindows),
        timeoutSeconds,
        statusMessage: normalizeOptionalText(row.statusMessage),
        groupAdvancedJson: normalizeOptionalText(row.groupAdvancedJson),
        advancedJson: normalizeOptionalText(row.advancedJson),
      }];
    });
  }

  async function saveHookSource() {
    if (hookSaving.value || !editingSource.value) return;
    hookError.value = null;
    hookSaving.value = true;
    try {
      const saved = await updateHookSource(editingSource.value, {
        handlers: buildHookHandlersInput(),
      });
      onSaved?.(saved);
      showHookEditor.value = false;
      await refresh();
    } catch (err) {
      hookError.value = err instanceof Error ? err.message : String(err);
    } finally {
      hookSaving.value = false;
    }
  }

  return {
    showHookEditor,
    editingSource,
    hookHandlerRows,
    hookSaving,
    hookError,
    hookEditorTitle,
    openHookEditor,
    addHookHandler,
    removeHookHandler,
    saveHookSource,
  };
}
