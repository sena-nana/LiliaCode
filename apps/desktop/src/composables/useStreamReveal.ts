import { ref } from "vue";
import type { ChatMessage } from "@lilia/contracts";

export type LocalMessage = ChatMessage & {
  streaming?: boolean;
};

export function useStreamReveal(
  messages: { value: LocalMessage[] },
  taskId: () => string,
) {
  const streamBuffer = ref("");
  const streamingId = ref<string | null>(null);
  let streamFinalized = false;
  let revealTimer: number | null = null;

  function startStreamBubble() {
    const bubble: LocalMessage = {
      id: `stream-${Date.now()}`,
      taskId: taskId(),
      role: "assistant",
      content: "",
      createdAt: Date.now(),
      streaming: true,
    };
    messages.value = [...messages.value, bubble];
    streamingId.value = bubble.id;
    streamBuffer.value = "";
    streamFinalized = false;
    ensureRevealLoop();
  }

  function ensureRevealLoop() {
    if (revealTimer !== null) return;
    revealTimer = window.setInterval(tickReveal, 24);
  }

  function stopRevealLoop() {
    if (revealTimer !== null) {
      window.clearInterval(revealTimer);
      revealTimer = null;
    }
  }

  function tickReveal() {
    if (!streamingId.value) {
      stopRevealLoop();
      return;
    }
    if (streamBuffer.value.length === 0) {
      if (streamFinalized) finalizeStream();
      return;
    }
    const n = Math.max(
      1,
      Math.min(streamBuffer.value.length, Math.ceil(streamBuffer.value.length / 20)),
    );
    const slice = streamBuffer.value.slice(0, n);
    streamBuffer.value = streamBuffer.value.slice(n);
    const idx = messages.value.findIndex((m) => m.id === streamingId.value);
    if (idx >= 0) {
      const m = messages.value[idx];
      messages.value[idx] = { ...m, content: m.content + slice };
    }
  }

  function finalizeStream() {
    const idx = messages.value.findIndex((m) => m.id === streamingId.value);
    if (idx >= 0) {
      messages.value[idx] = { ...messages.value[idx], streaming: false };
    }
    streamingId.value = null;
    streamFinalized = false;
    stopRevealLoop();
  }

  function abortStream() {
    if (streamingId.value) {
      messages.value = messages.value.filter((m) => m.id !== streamingId.value);
    }
    streamingId.value = null;
    streamBuffer.value = "";
    streamFinalized = false;
    stopRevealLoop();
  }

  function markStreamDone() {
    streamFinalized = true;
    ensureRevealLoop();
  }

  function appendChunk(text: string) {
    streamBuffer.value += text;
    ensureRevealLoop();
  }

  return {
    streamBuffer,
    streamingId,
    startStreamBubble,
    abortStream,
    markStreamDone,
    appendChunk,
    stopRevealLoop,
  };
}
