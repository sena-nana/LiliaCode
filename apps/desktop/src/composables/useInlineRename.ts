import { ref } from "vue";
import { useFocusOnActivation } from "./useFocusOnActivation";

export interface InlineRenameOptions {
  commit: (id: string, value: string) => void;
  currentId: () => string;
  currentValue: () => string;
}

export function useInlineRename(options: InlineRenameOptions) {
  const editingId = ref<string | null>(null);
  const editingValue = ref("");
  const editingInput = ref<HTMLInputElement | null>(null);
  useFocusOnActivation(editingInput, () => editingId.value === options.currentId(), true);

  function startRename() {
    editingId.value = options.currentId();
    editingValue.value = options.currentValue();
  }

  function resetRename() {
    editingId.value = null;
    editingValue.value = "";
  }

  function commitRename() {
    const id = editingId.value;
    if (!id) return;
    const next = editingValue.value.trim();
    if (next) options.commit(id, next);
    resetRename();
  }

  function cancelRename() {
    resetRename();
  }

  function onEditingKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      commitRename();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelRename();
    }
  }

  function bindEditingInput(el: unknown) {
    editingInput.value = (el as HTMLInputElement | null) ?? null;
  }

  return {
    editingId,
    editingValue,
    startRename,
    commitRename,
    cancelRename,
    onEditingKeydown,
    bindEditingInput,
  };
}

