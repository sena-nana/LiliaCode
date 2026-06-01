import { computed, ref, watch, type ComputedRef } from "vue";
import type { ToolConsentRequest, ToolConsentUpdatedInput } from "../services/chat";

export function useEditableToolCommand(
  request: ComputedRef<ToolConsentRequest | null>,
) {
  const commandDraft = ref("");
  const isEditingCommand = ref(false);

  const isBashCommand = computed(() => {
    const active = request.value;
    return active?.toolName === "Bash" && typeof active.input.command === "string";
  });

  const originalCommand = computed(() => {
    if (!isBashCommand.value) return "";
    return request.value?.input.command as string;
  });

  const hasEditableCommand = computed(() => isBashCommand.value);
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
