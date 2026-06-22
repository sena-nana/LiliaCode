<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch, type ComponentPublicInstance } from "vue";
import { useAnchoredOverlay } from "../composables/useAnchoredOverlay";
import { addDomEventListener, runUnlistenFns } from "../utils/eventListeners";

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
let listenerSeq = 0;
let documentUnlisteners: Array<() => void> = [];
let disposed = false;
const openState = computed(() => props.open);
const preferredPlacement = computed(() => "bottom-start" as const);
const {
  overlayEl: menuEl,
  overlayStyle,
  resolvedPlacement,
  containsTarget,
} = useAnchoredOverlay({
  open: openState,
  anchorEl: root,
  preferredPlacement,
  offset: 0,
  matchAnchorWidth: true,
});
void menuEl;
const menuPlacementClass = computed(() =>
  resolvedPlacement.value.startsWith("top")
    ? "search-dropdown__menu--top"
    : "search-dropdown__menu--bottom",
);

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
  if (!props.closeOnOutside) return;
  if (!containsTarget(event.target)) {
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

function setMenuEl(el: Element | ComponentPublicInstance | null) {
  menuEl.value = el instanceof HTMLElement ? el : null;
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

function clearDocumentListeners() {
  runUnlistenFns(documentUnlisteners.splice(0).reverse());
}

function installDocumentListeners() {
  clearDocumentListeners();
  const unlisteners: Array<() => void> = [];
  if (props.closeOnOutside) {
    unlisteners.push(addDomEventListener(document, "pointerdown", onDocPointer, true));
  }
  if (props.closeOnEscape) {
    unlisteners.push(addDomEventListener(document, "keydown", onDocKey));
  }
  documentUnlisteners = unlisteners;
}

watch(() => props.open, async (open) => {
  const seq = ++listenerSeq;
  clearDocumentListeners();
  if (open) {
    await nextTick();
    if (seq !== listenerSeq || !props.open) return;
    installDocumentListeners();
  }
});

onBeforeUnmount(() => {
  disposed = true;
  listenerSeq += 1;
  clearDocumentListeners();
});

defineExpose({
  focus: (options?: FocusOptions & { open?: boolean }) => {
    if (disposed) return;
    if (options?.open === false) {
      suppressNextFocusOpen.value = true;
    }
    inputRef.value?.focus(options);
    void nextTick(() => {
      if (disposed) return;
      suppressNextFocusOpen.value = false;
    });
  },
});
</script>

<template>
  <div
    ref="root"
    class="search-dropdown"
    :class="{
      'is-open': open,
      'is-open-top': open && resolvedPlacement.startsWith('top'),
      'is-open-bottom': open && !resolvedPlacement.startsWith('top'),
    }"
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
    <Teleport to="body">
      <div
        v-if="open"
        :ref="setMenuEl"
        class="search-dropdown__menu"
        :class="menuPlacementClass"
        role="listbox"
        :style="overlayStyle"
      >
        <slot
          :query="modelValue"
          :highlight-query-segments="highlightQuerySegments"
          :highlight-range-segments="highlightRangeSegments"
        />
      </div>
    </Teleport>
  </div>
</template>
