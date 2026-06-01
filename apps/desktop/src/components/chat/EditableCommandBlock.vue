<script setup lang="ts">
import { nextTick, ref, watch } from "vue";

const props = defineProps<{
  modelValue: string;
  editing: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  "begin-edit": [];
}>();

const commandInput = ref<HTMLTextAreaElement | null>(null);

watch(
  () => props.editing,
  (editing) => {
    if (!editing) return;
    void nextTick(() => {
      commandInput.value?.focus();
      commandInput.value?.select();
    });
  },
);

function updateCommand(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}
</script>

<template>
  <section
    class="editable-command"
    aria-label="完整命令"
  >
    <p class="timeline-card__label">COMMAND</p>
    <textarea
      v-if="editing"
      ref="commandInput"
      :value="modelValue"
      class="editable-command__input"
      rows="3"
      aria-label="编辑命令"
      @input="updateCommand"
    />
    <button
      v-else
      type="button"
      class="editable-command__preview timeline-code-block"
      aria-label="编辑完整命令"
      @click="emit('begin-edit')"
    >
      <code>{{ modelValue }}</code>
    </button>
    <p
      v-if="editing"
      class="editable-command__hint"
    >
      正在编辑命令。
    </p>
  </section>
</template>
