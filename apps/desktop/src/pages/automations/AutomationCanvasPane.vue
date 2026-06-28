<script setup lang="ts">
import "@vue-flow/core/dist/style.css";
import { computed, onMounted, watch } from "vue";
import { Handle, Position, VueFlow, useVueFlow } from "@vue-flow/core";
import type { AutomationNode, AutomationNodeKind } from "@lilia/contracts";
import { NODE_KIND_LABELS } from "./constants";
import type {
  AutomationFlowConnection,
  AutomationFlowEdge,
  AutomationFlowNode,
} from "./types";

interface MinimapRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const NODE_WIDTH = 184;
const NODE_HEIGHT = 78;
const MINIMAP_SIZE = { width: 148, height: 94 };
const MINIMAP_PADDING = 8;

const props = defineProps<{
  nodes: AutomationFlowNode[];
  edges: AutomationFlowEdge[];
  fitRequestKey: number;
  sourceHandlesForNode: (
    node: AutomationNode,
  ) => Array<{ id: string; label: string; top: string }>;
  onNodeClick: (event: { node: AutomationFlowNode }) => void;
  onConnect: (connection: AutomationFlowConnection) => void;
}>();

const emit = defineEmits<{
  "update:nodes": [nodes: AutomationFlowNode[]];
  "update:edges": [edges: AutomationFlowEdge[]];
}>();

const flowNodes = computed({
  get: () => props.nodes,
  set: (value: AutomationFlowNode[]) => emit("update:nodes", value),
});

const flowEdges = computed({
  get: () => props.edges,
  set: (value: AutomationFlowEdge[]) => emit("update:edges", value),
});

const { dimensions, fitView, zoomIn, zoomOut, viewport } = useVueFlow();

const canvasBackgroundStyle = computed(() => {
  const gridSize = 24 * viewport.value.zoom;
  return {
    "--automation-canvas-grid-size": `${gridSize}px`,
    "--automation-canvas-grid-x": `${viewport.value.x}px`,
    "--automation-canvas-grid-y": `${viewport.value.y}px`,
  };
});

const minimap = computed(() => {
  if (!flowNodes.value.length) return { nodes: [], viewport: null };
  const viewportZoom = Math.max(0.0001, viewport.value.zoom);
  const viewportRect = dimensions.value.width > 0 && dimensions.value.height > 0
    ? {
      x: -viewport.value.x / viewportZoom,
      y: -viewport.value.y / viewportZoom,
      width: dimensions.value.width / viewportZoom,
      height: dimensions.value.height / viewportZoom,
    }
    : null;
  const nodeRects = flowNodes.value.map((node) => ({
    id: node.id,
    status: node.data.status,
    rect: {
      x: node.position.x,
      y: node.position.y,
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
    },
  }));
  const rects = [
    ...nodeRects.map((node) => node.rect),
    ...(viewportRect ? [viewportRect] : []),
  ];
  const minX = Math.min(...rects.map((rect) => rect.x));
  const minY = Math.min(...rects.map((rect) => rect.y));
  const bounds = {
    x: minX,
    y: minY,
    width: Math.max(1, Math.max(...rects.map((rect) => rect.x + rect.width)) - minX),
    height: Math.max(1, Math.max(...rects.map((rect) => rect.y + rect.height)) - minY),
  };
  const scale = Math.min(
    (MINIMAP_SIZE.width - MINIMAP_PADDING * 2) / bounds.width,
    (MINIMAP_SIZE.height - MINIMAP_PADDING * 2) / bounds.height,
  );

  return {
    nodes: nodeRects.map((node) => ({
      id: node.id,
      status: node.status,
      style: rectToStyle(node.rect, bounds, scale),
    })),
    viewport: viewportRect ? rectToStyle(viewportRect, bounds, scale) : null,
  };
});

function percent(value: number, total: number): string {
  return `${(value / total) * 100}%`;
}

function rectToStyle(rect: MinimapRect, bounds: MinimapRect, scale: number) {
  const renderedWidth = bounds.width * scale;
  const renderedHeight = bounds.height * scale;
  const offsetX = (MINIMAP_SIZE.width - renderedWidth) / 2;
  const offsetY = (MINIMAP_SIZE.height - renderedHeight) / 2;
  return {
    left: percent(offsetX + (rect.x - bounds.x) * scale, MINIMAP_SIZE.width),
    top: percent(offsetY + (rect.y - bounds.y) * scale, MINIMAP_SIZE.height),
    width: percent(rect.width * scale, MINIMAP_SIZE.width),
    height: percent(rect.height * scale, MINIMAP_SIZE.height),
  };
}

function fitCanvas() {
  void fitView({ padding: 0.2 });
}

function zoomCanvasIn() {
  void zoomIn();
}

function zoomCanvasOut() {
  void zoomOut();
}

watch(
  () => props.fitRequestKey,
  (value, previous) => {
    if (value === previous) return;
    fitCanvas();
  },
);

onMounted(() => {
  if (props.fitRequestKey > 0) {
    fitCanvas();
  }
});
</script>

<template>
  <div class="automations-page__canvas" :style="canvasBackgroundStyle">
    <slot
      :fit-canvas="fitCanvas"
      :zoom-canvas-in="zoomCanvasIn"
      :zoom-canvas-out="zoomCanvasOut"
    />
    <div class="automations-page__minimap" role="img" aria-label="节点小地图">
      <span
        v-for="node in minimap.nodes"
        :key="node.id"
        class="automations-page__minimap-node"
        :class="node.status ? `is-${node.status}` : ''"
        :style="node.style"
      />
      <span
        v-if="minimap.viewport"
        class="automations-page__minimap-viewport"
        :style="minimap.viewport"
      />
    </div>
    <VueFlow
      v-model:nodes="flowNodes"
      v-model:edges="flowEdges"
      :default-viewport="{ x: 0, y: 0, zoom: 1 }"
      fit-view-on-init
      @node-click="props.onNodeClick"
      @connect="props.onConnect"
    >
      <template #node-automation="{ data }">
        <div class="automations-page__node" :class="{ 'is-selected': data.selected }">
          <Handle type="target" :position="Position.Left" class="automations-page__node-port" />
          <div class="automations-page__node-kind">
            {{ NODE_KIND_LABELS[data.node.kind as AutomationNodeKind] }}
          </div>
          <div class="automations-page__node-title">{{ data.node.title }}</div>
          <div v-if="data.status" class="automations-page__node-meta">{{ data.status }}</div>
          <div
            v-for="handle in props.sourceHandlesForNode(data.node as AutomationNode)"
            :key="handle.id"
            class="automations-page__node-handle"
            :style="{ top: handle.top }"
          >
            <span>{{ handle.label }}</span>
            <Handle
              type="source"
              :id="handle.id"
              :position="Position.Right"
              class="automations-page__node-port"
            />
          </div>
        </div>
      </template>
    </VueFlow>
  </div>
</template>

