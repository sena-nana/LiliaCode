<script lang="ts">
let mermaidConfigured = false;
let mermaidInstanceSeed = 0;

type MermaidApi = typeof import("mermaid")["default"];
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = defineProps<{
  blockKey: string;
  source: string;
}>();

const container = ref<HTMLElement | null>(null);
const state = ref<"idle" | "rendering" | "ready" | "error">("idle");
const errorText = ref("");
let renderId = 0;
let renderTimer: ReturnType<typeof window.setTimeout> | null = null;

const instanceId = `m${++mermaidInstanceSeed}`;
const MERMAID_RENDER_DELAY_MS = 80;
const MAX_MERMAID_SOURCE_LENGTH = 20_000;

async function getMermaid(): Promise<MermaidApi> {
  const module = await import("mermaid");
  return module.default;
}

function configureMermaid(mermaid: MermaidApi) {
  if (mermaidConfigured) return;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    themeVariables: {
      background: "transparent",
      mainBkg: "transparent",
      fontFamily: "var(--font-sans)",
      primaryColor: "transparent",
      primaryTextColor: "currentColor",
      lineColor: "currentColor",
      textColor: "currentColor",
    },
  });
  mermaidConfigured = true;
}

function clearRenderTimer() {
  if (renderTimer === null) return;
  window.clearTimeout(renderTimer);
  renderTimer = null;
}

function scheduleRenderDiagram() {
  const currentRenderId = renderId + 1;
  renderId = currentRenderId;
  clearRenderTimer();

  renderTimer = window.setTimeout(() => {
    renderTimer = null;
    void renderDiagram(currentRenderId);
  }, MERMAID_RENDER_DELAY_MS);
}

async function renderDiagram(currentRenderId: number) {
  const element = container.value;
  if (!element) return;

  const source = props.source.trim();
  element.innerHTML = "";
  errorText.value = "";

  if (!source) {
    state.value = "error";
    errorText.value = "Mermaid 内容为空。";
    return;
  }

  if (source.length > MAX_MERMAID_SOURCE_LENGTH) {
    state.value = "error";
    errorText.value = "Mermaid 内容过长，已跳过渲染。";
    return;
  }

  state.value = "rendering";
  await nextTick();

  try {
    const mermaid = await getMermaid();
    if (currentRenderId !== renderId || !container.value) return;
    configureMermaid(mermaid);
    const id = `markdown-mermaid-${instanceId}-${props.blockKey}-${currentRenderId}`.replace(
      /[^A-Za-z0-9_-]/g,
      "-",
    );
    const { svg, bindFunctions } = await mermaid.render(id, source);
    if (currentRenderId !== renderId || !container.value) return;
    container.value.innerHTML = svg;
    bindFunctions?.(container.value);
    state.value = "ready";
  } catch (error) {
    if (currentRenderId !== renderId || !container.value) return;
    container.value.innerHTML = "";
    state.value = "error";
    errorText.value = error instanceof Error
      ? error.message
      : "Mermaid 渲染失败。";
  }
}

watch(
  () => [props.source, props.blockKey] as const,
  () => {
    scheduleRenderDiagram();
  },
);

onMounted(() => {
  scheduleRenderDiagram();
});

onBeforeUnmount(() => {
  clearRenderTimer();
  renderId += 1;
});
</script>

<template>
  <figure
    class="markdown-block__mermaid"
    :class="`markdown-block__mermaid--${state}`"
  >
    <div ref="container" class="markdown-block__mermaid-canvas" />
    <figcaption v-if="state === 'rendering'" class="markdown-block__render-note">
      正在渲染 Mermaid…
    </figcaption>
    <figcaption v-else-if="state === 'error'" class="markdown-block__render-error">
      {{ errorText }}
    </figcaption>
  </figure>
</template>
