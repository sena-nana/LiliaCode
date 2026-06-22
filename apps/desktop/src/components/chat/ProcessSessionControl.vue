<script setup lang="ts">
import { computed, ref } from "vue";
import { Play, SendHorizontal, Square, Terminal } from "lucide-vue-next";

const props = defineProps<{
  running: boolean;
  busy: boolean;
  disabled?: boolean;
  error?: string | null;
}>();

const emit = defineEmits<{
  start: [command: string];
  "send-stdin": [stdin: string];
  stop: [];
}>();

const command = ref("");
const stdin = ref("");

const canStart = computed(() =>
  !props.running && !props.busy && props.disabled !== true
);
const canSendStdin = computed(() =>
  props.running && !props.busy && props.disabled !== true
);
const canStop = computed(() => props.running && !props.busy);

function submitStart() {
  if (!canStart.value) return;
  emit("start", command.value);
}

function submitStdin() {
  if (!canSendStdin.value) return;
  const value = stdin.value;
  emit("send-stdin", value);
  if (value) stdin.value = "";
}
</script>

<template>
  <section class="process-session-control" aria-label="进程会话">
    <form
      v-if="!running"
      class="process-session-control__row"
      @submit.prevent="submitStart"
    >
      <div class="process-session-control__status">
        <Terminal aria-hidden="true" />
        <span>进程</span>
      </div>
      <input
        v-model="command"
        class="process-session-control__input"
        type="text"
        :disabled="busy"
        placeholder="npm test -- --watch"
        aria-label="进程命令"
        autocomplete="off"
        spellcheck="false"
      />
      <button
        class="process-session-control__button process-session-control__button--primary"
        type="submit"
        :disabled="!canStart"
        title="启动进程会话"
        aria-label="启动进程会话"
      >
        <Play aria-hidden="true" />
        <span>启动</span>
      </button>
    </form>

    <form
      v-else
      class="process-session-control__row"
      @submit.prevent="submitStdin"
    >
      <div class="process-session-control__status is-running">
        <Terminal aria-hidden="true" />
        <span>运行中</span>
      </div>
      <input
        v-model="stdin"
        class="process-session-control__input"
        type="text"
        :disabled="busy"
        placeholder="stdin"
        aria-label="stdin"
        autocomplete="off"
        spellcheck="false"
      />
      <button
        class="process-session-control__button process-session-control__button--primary"
        type="submit"
        :disabled="!canSendStdin"
        title="发送 stdin"
        aria-label="发送 stdin"
      >
        <SendHorizontal aria-hidden="true" />
        <span>发送</span>
      </button>
      <button
        class="process-session-control__button process-session-control__button--danger"
        type="button"
        :disabled="!canStop"
        title="停止进程"
        aria-label="停止进程"
        @click="emit('stop')"
      >
        <Square aria-hidden="true" />
        <span>停止</span>
      </button>
    </form>

    <p v-if="error" class="process-session-control__error" role="status">
      {{ error }}
    </p>
  </section>
</template>
