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

const instanceId = `m${++mermaidInstanceSeed}`;

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

async function renderDiagram() {
  const element = container.value;
  if (!element) return;

  const source = props.source.trim();
  const currentRenderId = renderId + 1;
  renderId = currentRenderId;
  element.innerHTML = "";
  errorText.value = "";

  if (!source) {
    state.value = "error";
    errorText.value = "Mermaid 内容为空。";
    return;
  }

  state.value = "rendering";
  await nextTick();

  try {
    const mermaid = await getMermaid();
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
    void renderDiagram();
  },
);

onMounted(() => {
  void renderDiagram();
});

onBeforeUnmount(() => {
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
