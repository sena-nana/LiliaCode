<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import Chart from "chart.js/auto";
import type { ChartConfiguration, ChartData, ChartOptions, ChartType } from "chart.js";

const props = defineProps<{
  type: ChartType;
  data: ChartData;
  options?: ChartOptions;
  label: string;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const chart = shallowRef<Chart | null>(null);
let themeObserver: MutationObserver | null = null;
let renderSeq = 0;
let disposed = false;

function resolveCssColor(value: string) {
  const match = value.match(/^var\((--[\w-]+)(?:,\s*([^)]+))?\)$/);
  if (!match || typeof window === "undefined") return value;
  const cssValue = getComputedStyle(document.documentElement).getPropertyValue(match[1]).trim();
  return cssValue || match[2]?.trim() || value;
}

function resolveChartTokens<T>(value: T): T {
  if (typeof value === "string") return resolveCssColor(value) as T;
  if (Array.isArray(value)) return value.map((item) => resolveChartTokens(item)) as T;
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [key, resolveChartTokens(item)]),
  ) as T;
}

function destroyChart() {
  chart.value?.destroy();
  chart.value = null;
}

function renderChart(seq = renderSeq) {
  if (disposed || seq !== renderSeq) return;
  const canvas = canvasRef.value;
  if (!canvas) return;
  destroyChart();
  const config: ChartConfiguration = {
    type: props.type,
    data: resolveChartTokens(props.data),
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: false,
      ...resolveChartTokens(props.options ?? {}),
    },
  };
  chart.value = new Chart(canvas, config);
}

function scheduleRender() {
  const seq = ++renderSeq;
  void nextTick(() => renderChart(seq));
}

watch(
  () => [props.type, props.data, props.options],
  scheduleRender,
  { deep: true },
);

onMounted(() => {
  disposed = false;
  renderChart();
  if (typeof MutationObserver !== "undefined") {
    themeObserver = new MutationObserver(scheduleRender);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class", "style", "data-theme"],
    });
  }
});

onBeforeUnmount(() => {
  disposed = true;
  renderSeq += 1;
  themeObserver?.disconnect();
  destroyChart();
});
</script>

<template>
  <div class="quota-chart-canvas" role="img" :aria-label="label">
    <canvas ref="canvasRef" aria-hidden="true" />
  </div>
</template>

<style scoped>
.quota-chart-canvas {
  position: relative;
  min-width: 0;
  width: 100%;
  height: 100%;
}

.quota-chart-canvas canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
