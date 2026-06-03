<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

defineProps<{
  placeholder: string;
  isEmpty: boolean;
}>();

const emit = defineEmits<{
  editor: [element: HTMLDivElement | null];
  input: [];
  selection: [];
  keydown: [event: KeyboardEvent];
  paste: [event: ClipboardEvent];
}>();

const editor = ref<HTMLDivElement | null>(null);

onMounted(() => {
  emit("editor", editor.value);
});

onBeforeUnmount(() => {
  emit("editor", null);
});
</script>

<template>
  <div
    ref="editor"
    class="chat-composer__rich-input"
    :class="{ 'is-empty': isEmpty }"
    role="textbox"
    aria-multiline="true"
    contenteditable="true"
    spellcheck="false"
    :data-placeholder="placeholder"
    @input="emit('input')"
    @click="emit('selection')"
    @keyup="emit('selection')"
    @mouseup="emit('selection')"
    @keydown="emit('keydown', $event)"
    @paste="emit('paste', $event)"
  ></div>
</template>
