<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  projectArchitectureChangeText,
  type ProjectArchitectureChangeRecord,
  type ProjectArchitectureGraph,
} from "@lilia/contracts";
import {
  getProjectArchitecture,
  listProjectArchitectureChanges,
  rollbackProjectArchitecture,
} from "../../services/chat";
import { useConnectionStatus } from "../../composables/useConnectionStatus";
import { withComponentEpoch } from "@lilia/ui";
import MarkdownMermaid from "./MarkdownMermaid.vue";

const props = defineProps<{
  taskId: string;
  projectId?: string;
  projectCwd: string | null;
}>();

const { activeBackend } = useConnectionStatus({ probe: false });
const graph = ref<ProjectArchitectureGraph | null>(null);
const changes = ref<ProjectArchitectureChangeRecord[]>([]);
const loading = ref(false);
const error = ref("");
const rollingBack = ref(false);
const panelEpoch = withComponentEpoch();

const hasGraphContent = computed(() =>
  Boolean(graph.value && (graph.value.nodes.length > 0 || graph.value.edges.length > 0 || graph.value.summary)),
);
const mermaidSource = computed(() => buildMermaid(graph.value));
const recentChanges = computed(() => changes.value.slice(0, 8));
const changeTimeFormatter = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

async function loadArchitecture() {
  if (!panelEpoch.assertAlive()) return;
  const seq = panelEpoch.nextEpoch();
  if (!props.projectId) {
    graph.value = null;
    changes.value = [];
    loading.value = false;
    error.value = "";
    return;
  }
  loading.value = true;
  error.value = "";
  const projectId = props.projectId;
  try {
    const [nextGraph, nextChanges] = await Promise.all([
      getProjectArchitecture(projectId),
      listProjectArchitectureChanges(projectId, 20),
    ]);
    if (!panelEpoch.assertAlive(seq)) return;
    graph.value = nextGraph;
    changes.value = nextChanges;
  } catch (err) {
    if (!panelEpoch.assertAlive(seq)) return;
    error.value = String(err);
  } finally {
    if (!panelEpoch.assertAlive(seq)) return;
    loading.value = false;
  }
}

async function rollbackPreviousVersion() {
  if (!panelEpoch.assertAlive() || !props.projectId || rollingBack.value) return;
  rollingBack.value = true;
  error.value = "";
  const projectId = props.projectId;
  const taskId = props.taskId;
  try {
    await rollbackProjectArchitecture(projectId, taskId, activeBackend.value);
    if (!panelEpoch.assertAlive() || projectId !== props.projectId || taskId !== props.taskId) return;
    await loadArchitecture();
  } catch (err) {
    if (!panelEpoch.assertAlive()) return;
    error.value = String(err);
  } finally {
    if (!panelEpoch.assertAlive()) return;
    rollingBack.value = false;
  }
}

function buildMermaid(value: ProjectArchitectureGraph | null): string {
  if (!value || (value.nodes.length === 0 && value.edges.length === 0)) return "";
  const lines = ["flowchart TD"];
  for (const node of value.nodes) {
    lines.push(`  ${mermaidId(node.id)}["${escapeMermaid(node.label || node.id)}"]`);
  }
  const nodeIds = new Set(value.nodes.map((node) => node.id));
  for (const edge of value.edges) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) continue;
    const label = edge.label || edge.type;
    lines.push(
      `  ${mermaidId(edge.from)} -->${label ? `|"${escapeMermaid(label)}"|` : ""} ${mermaidId(edge.to)}`,
    );
  }
  return lines.join("\n");
}

function mermaidId(value: string): string {
  const normalized = value.replace(/[^A-Za-z0-9_]/g, "_");
  return /^[A-Za-z_]/.test(normalized) ? normalized : `n_${normalized}`;
}

function escapeMermaid(value: string): string {
  return value.replace(/"/g, "'").replace(/\n/g, " ");
}

function eventSummary(event: ProjectArchitectureChangeRecord): string {
  return event.reason ||
    event.changes.map((change) => projectArchitectureChangeText(change)).slice(0, 2).join("；") ||
    event.status;
}

function eventVersionLabel(event: ProjectArchitectureChangeRecord): string {
  const before = `v${event.beforeVersion}`;
  const after = event.afterVersion == null ? "无新版本" : `v${event.afterVersion}`;
  return `版本：${before} -> ${after}`;
}

function eventTimeLabel(event: ProjectArchitectureChangeRecord): string {
  if (!event.createdAt) return "时间：未知";
  return `时间：${changeTimeFormatter.format(new Date(event.createdAt))}`;
}

onMounted(loadArchitecture);
watch(() => props.projectId, loadArchitecture);
</script>

<template>
  <div class="architecture-panel">
    <div v-if="loading" class="architecture-panel__empty">正在加载架构图…</div>
    <div v-else-if="error" class="architecture-panel__empty architecture-panel__empty--error">
      {{ error }}
    </div>
    <div v-else-if="!hasGraphContent" class="architecture-panel__empty">
      暂无架构图，后续对话涉及架构时会逐步补全
    </div>
    <div v-else class="architecture-panel__content">
      <header class="architecture-panel__header">
        <div>
          <p class="architecture-panel__eyebrow">版本 {{ graph?.version ?? 0 }}</p>
          <h2 class="architecture-panel__title">项目架构图</h2>
        </div>
        <button
          type="button"
          class="ui-button ui-button--ghost architecture-panel__rollback"
          data-agent-id="architecture.rollback"
          :disabled="rollingBack || changes.length === 0"
          @click="rollbackPreviousVersion"
        >
          回滚上一版本
        </button>
      </header>

      <p v-if="graph?.summary" class="architecture-panel__summary">{{ graph.summary }}</p>

      <MarkdownMermaid
        v-if="mermaidSource"
        block-key="project-architecture"
        :source="mermaidSource"
      />

      <section class="architecture-panel__section" aria-label="节点">
        <h3 class="architecture-panel__section-title">节点</h3>
        <article
          v-for="node in graph?.nodes ?? []"
          :key="node.id"
          class="architecture-panel__item"
        >
          <div class="architecture-panel__item-head">
            <strong>{{ node.label || node.id }}</strong>
            <span>{{ node.type }}</span>
          </div>
          <p v-if="node.summary">{{ node.summary }}</p>
          <p v-if="node.paths.length" class="architecture-panel__meta">
            {{ node.paths.join(" · ") }}
          </p>
        </article>
      </section>

      <section class="architecture-panel__section" aria-label="关系">
        <h3 class="architecture-panel__section-title">关系</h3>
        <article
          v-for="edge in graph?.edges ?? []"
          :key="edge.id"
          class="architecture-panel__item"
        >
          <div class="architecture-panel__item-head">
            <strong>{{ edge.label || edge.id }}</strong>
            <span>{{ edge.type }}</span>
          </div>
          <p class="architecture-panel__meta">{{ edge.from }} → {{ edge.to }}</p>
          <p v-if="edge.summary">{{ edge.summary }}</p>
        </article>
      </section>

      <section class="architecture-panel__section" aria-label="最近变更">
        <h3 class="architecture-panel__section-title">最近变更</h3>
        <article
          v-for="event in recentChanges"
          :key="event.id ?? `${event.createdAt}`"
          class="architecture-panel__change"
        >
          <div class="architecture-panel__change-head">
            <span class="architecture-panel__change-status">{{ event.status }}</span>
            <span class="architecture-panel__change-backend">backend：{{ event.backend }}</span>
          </div>
          <p>{{ eventSummary(event) }}</p>
          <p class="architecture-panel__change-meta">
            <span>{{ eventTimeLabel(event) }}</span>
            <span>{{ eventVersionLabel(event) }}</span>
          </p>
        </article>
      </section>
    </div>
  </div>
</template>

