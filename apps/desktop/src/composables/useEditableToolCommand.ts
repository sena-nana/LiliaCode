import { computed, ref, watch, type ComputedRef } from "vue";
import {
  createUpdatedToolConsentCommandInput,
  readEditableToolConsentCommand,
  type ToolConsentRequest,
  type ToolConsentUpdatedInput,
} from "@lilia/contracts";

export function useEditableToolCommand(
  request: ComputedRef<ToolConsentRequest | null>,
) {
  const commandDraft = ref("");
  const isEditingCommand = ref(false);

  const originalCommand = computed(() => readEditableToolConsentCommand(request.value));

  const hasEditableCommand = computed(() => !!originalCommand.value);
  const commandChanged = computed(() =>
    hasEditableCommand.value && commandDraft.value !== originalCommand.value,
  );
  const commandIsEmpty = computed(() =>
    hasEditableCommand.value && commandDraft.value.trim().length === 0,
  );
  const updatedCommandInput = computed<ToolConsentUpdatedInput | undefined>(() => {
    if (!commandChanged.value || commandIsEmpty.value) return undefined;
    return createUpdatedToolConsentCommandInput(request.value, commandDraft.value);
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

