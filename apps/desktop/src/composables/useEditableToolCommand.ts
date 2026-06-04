import { computed, ref, watch, type ComputedRef } from "vue";
import type { ToolConsentRequest, ToolConsentUpdatedInput } from "../services/chat";

function stringifyCodexCommand(value: unknown): string {
  if (Array.isArray(value)) {
    return value
      .map((part) => {
        if (typeof part === "string") return part;
        if (part && typeof part === "object" && !Array.isArray(part)) {
          const row = part as Record<string, unknown>;
          return stringValue(row.text) ||
            stringValue(row.value) ||
            stringValue(row.arg) ||
            stringValue(row.command) ||
            "";
        }
        return stringValue(part) || "";
      })
      .filter(Boolean)
      .join(" ");
  }
  if (typeof value === "string") return value;
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const row = value as Record<string, unknown>;
    return stringifyCodexCommand(row.parsedCmd || row.command || row.cmd || row.args);
  }
  return "";
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function useEditableToolCommand(
  request: ComputedRef<ToolConsentRequest | null>,
) {
  const commandDraft = ref("");
  const isEditingCommand = ref(false);

  const isBashCommand = computed(() => {
    const active = request.value;
    return active?.toolName === "Bash" && typeof active.input.command === "string";
  });

  const codexCommand = computed(() => {
    const active = request.value;
    if (active?.backend !== "codex") return "";
    if (
      active.toolName !== "item/commandExecution/requestApproval" &&
      active.toolName !== "commandExecution"
    ) {
      return "";
    }
    return stringifyCodexCommand(
      active.input.parsedCmd ||
        active.input.command ||
        active.input.cmd ||
        active.input.commandActions ||
        active.commandActions,
    );
  });

  const originalCommand = computed(() => {
    if (isBashCommand.value) return request.value?.input.command as string;
    return codexCommand.value;
  });

  const hasEditableCommand = computed(() => isBashCommand.value || !!codexCommand.value);
  const commandChanged = computed(() =>
    hasEditableCommand.value && commandDraft.value !== originalCommand.value,
  );
  const commandIsEmpty = computed(() =>
    hasEditableCommand.value && commandDraft.value.trim().length === 0,
  );
  const updatedCommandInput = computed<ToolConsentUpdatedInput | undefined>(() => {
    const active = request.value;
    if (!active || !commandChanged.value || commandIsEmpty.value) return undefined;
    return { ...active.input, command: commandDraft.value };
  });

  function beginCommandEdit() {
    if (!hasEditableCommand.value) return;
    commandDraft.value = originalCommand.value;
    isEditingCommand.value = true;
  }

  function cancelCommandEdit() {
    commandDraft.value = originalCommand.value;
    isEditingCommand.value = false;
  }

  watch(
    () => [request.value?.requestId ?? "", originalCommand.value],
    () => {
      commandDraft.value = originalCommand.value;
      isEditingCommand.value = false;
    },
    { immediate: true },
  );

  return {
    commandDraft,
    isEditingCommand,
    hasEditableCommand,
    commandChanged,
    commandIsEmpty,
    updatedCommandInput,
    beginCommandEdit,
    cancelCommandEdit,
  };
}
