<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

interface Segment {
  text: string;
  mark: boolean;
}

const props = withDefaults(defineProps<{
  modelValue: string;
  open?: boolean;
  placeholder?: string;
  closeOnOutside?: boolean;
  closeOnEscape?: boolean;
  spellcheck?: boolean;
}>(), {
  open: true,
  placeholder: "",
  closeOnOutside: false,
  closeOnEscape: false,
  spellcheck: false,
});

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "update:open": [value: boolean];
  "open-request": [];
  input: [event: Event];
  keydown: [event: KeyboardEvent];
}>();

const root = ref<HTMLElement | null>(null);
const inputRef = ref<HTMLInputElement | null>(null);
const suppressNextFocusOpen = ref(false);

function requestOpen(event?: Event) {
  if (event?.type === "focus" && suppressNextFocusOpen.value) {
    suppressNextFocusOpen.value = false;
    return;
  }
  emit("update:open", true);
  emit("open-request");
}

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
  emit("input", event);
}

function onDocPointer(event: PointerEvent) {
  if (!props.closeOnOutside || !root.value) return;
  if (!root.value.contains(event.target as Node)) {
    emit("update:open", false);
  }
}

function onDocKey(event: KeyboardEvent) {
  if (event.key === "Escape" && props.open) {
    if (props.closeOnEscape) {
      emit("update:open", false);
    }
    event.stopPropagation();
  }
}

function highlightQuerySegments(text: string, query = props.modelValue.trim()): Segment[] {
  if (!query) return [{ text, mark: false }];
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const segments: Segment[] = [];
  let cursor = 0;
  let index = lowerText.indexOf(lowerQuery);

  while (index !== -1) {
    if (cursor < index) {
      segments.push({ text: text.slice(cursor, index), mark: false });
    }
    const end = index + query.length;
    segments.push({ text: text.slice(index, end), mark: true });
    cursor = end;
    index = lowerText.indexOf(lowerQuery, cursor);
  }

  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), mark: false });
  }
  return segments.length ? segments : [{ text, mark: false }];
}

function highlightRangeSegments(text: string, ranges: Array<[number, number]>): Segment[] {
  if (!ranges.length) return [{ text, mark: false }];
  const merged: Array<[number, number]> = [];
  for (const [start, end] of [...ranges].sort((a, b) => a[0] - b[0])) {
    const last = merged[merged.length - 1];
    if (last && start <= last[1]) {
      last[1] = Math.max(last[1], end);
    } else {
      merged.push([start, end]);
    }
  }

  const segments: Segment[] = [];
  let cursor = 0;
  for (const [start, end] of merged) {
    if (cursor < start) {
      segments.push({ text: text.slice(cursor, start), mark: false });
    }
    segments.push({ text: text.slice(start, end), mark: true });
    cursor = end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), mark: false });
  }
  return segments;
}

watch(() => props.open, async (open) => {
  if (open) {
    await nextTick();
    if (props.closeOnOutside) {
      document.addEventListener("pointerdown", onDocPointer, true);
    }
    if (props.closeOnEscape) {
      document.addEventListener("keydown", onDocKey);
    }
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onDocKey);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointer, true);
  document.removeEventListener("keydown", onDocKey);
});

defineExpose({
  focus: (options?: FocusOptions & { open?: boolean }) => {
    if (options?.open === false) {
      suppressNextFocusOpen.value = true;
    }
    inputRef.value?.focus(options);
    void nextTick(() => {
      suppressNextFocusOpen.value = false;
    });
  },
});
</script>

<template>
  <div
    ref="root"
    class="search-dropdown"
    :class="{ 'is-open': open }"
  >
    <div class="search-dropdown__field">
      <slot name="leading" />
      <input
        ref="inputRef"
        :value="modelValue"
        type="text"
        class="search-dropdown__input"
        :placeholder="placeholder"
        :spellcheck="spellcheck"
        @pointerdown="requestOpen"
        @focus="requestOpen"
        @input="onInput"
        @keydown="emit('keydown', $event)"
      />
      <slot name="trailing" />
    </div>

    <div
      v-if="open"
      class="search-dropdown__menu"
      role="listbox"
    >
      <slot
        :query="modelValue"
        :highlight-query-segments="highlightQuerySegments"
        :highlight-range-segments="highlightRangeSegments"
      />
    </div>
  </div>
</template>
