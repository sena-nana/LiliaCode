<script lang="ts">
let mermaidInstanceSeed = 0;
const MAX_MERMAID_SOURCE_LENGTH = 20_000;
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  beginPerfStage,
  cancelIdleRun,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";
import { useDeferredVisibility } from "./markdown/useDeferredVisibility";
import {
  MERMAID_EXPLICIT_RENDER_LENGTH,
  mermaidErrorMessage,
  needsExplicitMermaidActivation,
  renderMermaidDiagram,
} from "./markdown/mermaidRenderer";

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
let cancelPaintRender: (() => void) | null = null;
let disposed = false;

const instanceId = `m${++mermaidInstanceSeed}`;
const { activated } = useDeferredVisibility({
  target: () => figure.value,
  perfName: "markdown.mermaid.visible",
  detail: () => `${props.source.length} chars`,
});

function activationButtonLabel(source: string): string {
  return source.trim().length >= MERMAID_EXPLICIT_RENDER_LENGTH
    ? "图表较大，点击渲染。"
    : "点击渲染图表。";
}

function cancelScheduledRender() {
  cancelPaintRender?.();
  cancelPaintRender = null;
  if (renderIdleHandle === null) return;
  cancelIdleRun(renderIdleHandle);
  renderIdleHandle = null;
}

function resetDiagramState() {
  if (disposed) return;
  state.value = "idle";
  errorText.value = "";
  if (container.value) container.value.innerHTML = "";
}

function requestDiagramActivation() {
  if (disposed || activatedByUser.value) return;
  const stage = beginPerfStage("markdown.mermaid.activate", {
    detail: `${props.source.length} chars`,
  });
  activatedByUser.value = true;
  stage.end();
}

function scheduleRenderDiagram() {
  if (disposed) return;
  const currentRenderId = renderId + 1;
  renderId = currentRenderId;
  cancelScheduledRender();
  resetDiagramState();
  if (import.meta.env.MODE === "test") {
    void renderDiagram(currentRenderId);
    return;
  }
  cancelPaintRender = scheduleAfterPaint(() => {
    cancelPaintRender = null;
    if (currentRenderId !== renderId) return;
    renderIdleHandle = runWhenIdle(() => {
      renderIdleHandle = null;
      if (currentRenderId !== renderId) return;
      void renderDiagram(currentRenderId);
    });
  });
}

async function renderDiagram(currentRenderId: number) {
  if (disposed || currentRenderId !== renderId) return;
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
  if (disposed || currentRenderId !== renderId || !container.value) return;

  try {
    const id = `markdown-mermaid-${instanceId}-${props.blockKey}-${currentRenderId}`.replace(
      /[^A-Za-z0-9_-]/g,
      "-",
    );
    const { svg, bindFunctions } = await renderMermaidDiagram(id, source);
    if (disposed || currentRenderId !== renderId || !container.value) return;
    container.value.innerHTML = svg;
    bindFunctions?.(container.value);
    state.value = "ready";
  } catch (error) {
    if (disposed || currentRenderId !== renderId || !container.value) return;
    container.value.innerHTML = "";
    state.value = "error";
    errorText.value = mermaidErrorMessage(error);
  }
}

watch(
  () => [props.source, props.blockKey, activated.value, activatedByUser.value] as const,
  ([source, _blockKey, visible, activatedExplicitly]) => {
    const explicitActivationRequired = needsExplicitMermaidActivation(source);
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
  disposed = true;
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
        v-if="needsExplicitMermaidActivation(source) && !activatedByUser"
        type="button"
        class="markdown-block__render-activate"
        :data-agent-id="`markdown.mermaid.activate.${blockKey}`"
        @click="requestDiagramActivation"
      >
        {{ activationButtonLabel(source) }}
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
