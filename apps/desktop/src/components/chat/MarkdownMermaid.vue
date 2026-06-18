<script lang="ts">
let mermaidConfigured = false;
let mermaidInstanceSeed = 0;
let mermaidModuleLoad: Promise<typeof import("mermaid")> | null = null;

type MermaidApi = typeof import("mermaid")["default"];
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";
import { useDeferredVisibility } from "./markdown/useDeferredVisibility";

const props = defineProps<{
  blockKey: string;
  source: string;
}>();

const figure = ref<HTMLElement | null>(null);
const container = ref<HTMLElement | null>(null);
const state = ref<"idle" | "rendering" | "ready" | "error">("idle");
const errorText = ref("");
const activatedByUser = ref(false);
let renderId = 0;
let renderIdleHandle: number | null = null;

const instanceId = `m${++mermaidInstanceSeed}`;
const MAX_MERMAID_SOURCE_LENGTH = 20_000;
const EXPLICIT_MERMAID_RENDER_LENGTH = 1_200;
const { activated } = useDeferredVisibility({
  target: () => figure.value,
  perfName: "markdown.mermaid.visible",
  detail: () => `${props.source.length} chars`,
});

function needsExplicitActivation(source: string): boolean {
  return source.trim().length >= EXPLICIT_MERMAID_RENDER_LENGTH;
}

async function getMermaid(): Promise<MermaidApi> {
  if (!mermaidModuleLoad) {
    mermaidModuleLoad = measurePerfAsync(
      "markdown.mermaid.module.load",
      async () => await import("mermaid"),
    ).catch((err) => {
      mermaidModuleLoad = null;
      throw err;
    });
  }
  const module = await mermaidModuleLoad;
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

function cancelScheduledRender() {
  if (renderIdleHandle === null) return;
  cancelIdleRun(renderIdleHandle);
  renderIdleHandle = null;
}

function resetDiagramState() {
  state.value = "idle";
  errorText.value = "";
  if (container.value) container.value.innerHTML = "";
}

function requestDiagramActivation() {
  if (activatedByUser.value) return;
  const stage = beginPerfStage("markdown.mermaid.activate", {
    detail: `${props.source.length} chars`,
  });
  activatedByUser.value = true;
  stage.end();
}

function scheduleRenderDiagram() {
  const currentRenderId = renderId + 1;
  renderId = currentRenderId;
  cancelScheduledRender();
  resetDiagramState();
  if (import.meta.env.MODE === "test") {
    void renderDiagram(currentRenderId);
    return;
  }
  scheduleAfterPaint(() => {
    if (currentRenderId !== renderId) return;
    renderIdleHandle = runWhenIdle(() => {
      renderIdleHandle = null;
      if (currentRenderId !== renderId) return;
      void renderDiagram(currentRenderId);
    });
  });
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
    const { svg, bindFunctions } = await measurePerfAsync(
      "markdown.mermaid.render",
      async () => await mermaid.render(id, source),
      { detail: `${source.length} chars` },
    );
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
  () => [props.source, props.blockKey, activated.value, activatedByUser.value] as const,
  ([source, _blockKey, visible, activatedExplicitly]) => {
    const explicitActivationRequired = needsExplicitActivation(source);
    if (!source.trim()) {
      activatedByUser.value = false;
      resetDiagramState();
      cancelScheduledRender();
      return;
    }
    if (!visible) {
      resetDiagramState();
      cancelScheduledRender();
      return;
    }
    if (explicitActivationRequired && !activatedExplicitly) {
      resetDiagramState();
      cancelScheduledRender();
      return;
    }
    scheduleRenderDiagram();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelScheduledRender();
  renderId += 1;
});
</script>

<template>
  <figure
    ref="figure"
    class="markdown-block__mermaid"
    :class="`markdown-block__mermaid--${state}`"
  >
    <div ref="container" class="markdown-block__mermaid-canvas" />
    <figcaption v-if="state === 'idle'" class="markdown-block__render-note">
      <button
        v-if="needsExplicitActivation(source) && !activatedByUser"
        type="button"
        class="markdown-block__render-activate"
        @click="requestDiagramActivation"
      >
        图表较大，点击渲染。
      </button>
      <span v-else>图表进入可见区域后渲染。</span>
    </figcaption>
    <figcaption v-else-if="state === 'rendering'" class="markdown-block__render-note">
      正在渲染 Mermaid…
    </figcaption>
    <figcaption v-else-if="state === 'error'" class="markdown-block__render-error">
      {{ errorText }}
    </figcaption>
  </figure>
</template>

<style scoped>
.markdown-block__render-activate {
  border: 0;
  padding: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}
</style>
