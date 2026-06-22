<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import type { ChartConfiguration, ChartData, ChartOptions, ChartType } from "chart.js";
import { createLazyLoadState } from "../../utils/lazyLoadState";
import { measurePerfAsync } from "../../utils/perf";

type ChartConstructor = typeof import("chart.js/auto")["default"];

const props = defineProps<{
  type: ChartType;
  data: ChartData;
  options?: ChartOptions;
  label: string;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const chart = shallowRef<InstanceType<ChartConstructor> | null>(null);
const chartLoad = createLazyLoadState<ChartConstructor>(() =>
  measurePerfAsync(
    "quota.chart.module.load",
    async () => (await import("chart.js/auto")).default,
    { detail: props.label },
  )
);
const chartStatus = chartLoad.status;
const chartError = computed(() => {
  const error = chartLoad.error.value;
  return error instanceof Error ? error.message : String(error ?? "");
});
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

async function renderChart(seq = renderSeq) {
  if (disposed || seq !== renderSeq) return;
  const canvas = canvasRef.value;
  if (!canvas) return;
  destroyChart();
  const Chart = await chartLoad.load();
  if (disposed || seq !== renderSeq || canvasRef.value !== canvas) return;
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
  void nextTick(() => {
    void renderChart(seq).catch(() => undefined);
  });
}

function retryChartLoad() {
  scheduleRender();
}

watch(
  () => [props.type, props.data, props.options],
  scheduleRender,
  { deep: true },
);

onMounted(() => {
  disposed = false;
  void renderChart().catch(() => undefined);
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
  <div
    class="quota-chart-canvas"
    :class="`quota-chart-canvas--${chartStatus}`"
    role="img"
    :aria-label="label"
  >
    <canvas ref="canvasRef" aria-hidden="true" />
    <div v-if="chartStatus === 'error'" class="quota-chart-canvas__error">
      <span>{{ chartError || "图表加载失败。" }}</span>
      <button type="button" class="quota-chart-canvas__retry" @click="retryChartLoad">
        重试
      </button>
    </div>
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

.quota-chart-canvas--error canvas {
  opacity: 0.18;
}

.quota-chart-canvas__error {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  gap: 8px;
  padding: 12px;
  color: var(--text-secondary);
  font-size: 12px;
  text-align: center;
}

.quota-chart-canvas__retry {
  justify-self: center;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  padding: 4px 10px;
  background: var(--surface-2);
  color: var(--text-primary);
  font: inherit;
  cursor: pointer;
}
</style>
