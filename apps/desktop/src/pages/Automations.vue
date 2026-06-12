<script setup lang="ts">
import "../styles/pages/automations.css";
import "@vue-flow/core/dist/style.css";
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { Handle, Position, VueFlow, useVueFlow } from "@vue-flow/core";
import {
  Bot,
  Braces,
  Check,
  CircleHelp,
  GitBranch,
  Loader2,
  Maximize2,
  Minus,
  Play,
  Plus,
  Save,
  ToggleLeft,
  ToggleRight,
  Wrench,
  Zap,
} from "lucide-vue-next";
import type {
  AutomationEdge,
  AutomationNode,
  AutomationNodeKind,
  AutomationRun,
  AutomationRunNodeState,
  AutomationScopeFilter,
  AutomationWorkflow,
  ChatBackendKind,
  Project,
} from "@lilia/contracts";
import {
  getAutomationRun,
  listAutomationRuns,
  listAutomations,
  onAutomationChanged,
  onAutomationRunUpdated,
  publishAutomation,
  resumeAutomationRun,
  runAutomationOnce,
  saveAutomationDraft,
  setAutomationEnabled,
  type AutomationRunDetail,
} from "../services/automations";
import { ensureProjectsLoaded, listProjects } from "../services/projectsStore";

const DEFAULT_SCOPE: AutomationScopeFilter = {
  projectIds: [],
  includeInbox: true,
  taskStatuses: [],
  backends: [],
  eventKinds: [],
};

const NODE_KIND_LABELS: Record<AutomationNodeKind, string> = {
  trigger: "事件触发",
  agent: "Agent 调用",
  logic: "逻辑",
  tool: "工具",
  human: "人工确认",
};

const NODE_ICONS: Record<AutomationNodeKind, unknown> = {
  trigger: Zap,
  agent: Bot,
  logic: GitBranch,
  tool: Wrench,
  human: CircleHelp,
};

interface FlowNode {
  id: string;
  type: string;
  position: { x: number; y: number };
  data: {
    node: AutomationNode;
    selected: boolean;
    status: string | null;
  };
}

interface FlowEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  animated?: boolean;
}

interface FlowConnection {
  source: string | null;
  target: string | null;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

const workflowRows = ref<AutomationWorkflow[]>([]);
const projectRows = ref<Project[]>([]);
const selectedWorkflowId = ref<string | null>(null);
const selectedNodeId = ref<string | null>(null);
const nodes = shallowRef<FlowNode[]>([]);
const edges = shallowRef<FlowEdge[]>([]);
const workflowName = ref("新自动化");
const scope = ref<AutomationScopeFilter>({ ...DEFAULT_SCOPE });
const loading = ref(false);
const saving = ref(false);
const publishing = ref(false);
const running = ref(false);
const resuming = ref(false);
const errorText = ref<string | null>(null);
const runs = ref<AutomationRun[]>([]);
const selectedRunId = ref<string | null>(null);
const selectedRunDetail = ref<AutomationRunDetail | null>(null);
const selectedRunNodeId = ref<string | null>(null);
const unlisteners = ref<Array<() => void>>([]);

const { fitView, zoomIn, zoomOut } = useVueFlow();

const selectedWorkflow = computed(() =>
  workflowRows.value.find((workflow) => workflow.id === selectedWorkflowId.value) ?? null,
);

const selectedNode = computed(() =>
  nodes.value.find((node) => node.id === selectedNodeId.value) ?? null,
);

const selectedRunNodeStates = computed<AutomationRunNodeState[]>(() =>
  selectedRunDetail.value?.nodes ?? [],
);

const selectedRunNodeState = computed<AutomationRunNodeState | null>(() =>
  selectedRunNodeStates.value.find((state) => state.nodeId === selectedRunNodeId.value) ?? null,
);

const hasTriggerNode = computed(() =>
  nodes.value.some((node) => node.data.node.kind === "trigger"),
);

const minimapNodes = computed(() => {
  if (!nodes.value.length) return [];
  const xs = nodes.value.map((node) => node.position.x);
  const ys = nodes.value.map((node) => node.position.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  const maxX = Math.max(...xs) + 184;
  const maxY = Math.max(...ys) + 78;
  const width = Math.max(1, maxX - minX);
  const height = Math.max(1, maxY - minY);
  return nodes.value.map((node) => ({
    id: node.id,
    left: `${((node.position.x - minX) / width) * 100}%`,
    top: `${((node.position.y - minY) / height) * 100}%`,
    width: `${Math.max(8, (184 / width) * 100)}%`,
    height: `${Math.max(8, (78 / height) * 100)}%`,
    selected: node.id === selectedNodeId.value,
    status: nodeStatus(node.id),
  }));
});

const TASK_STATUS_OPTIONS = ["waiting", "running", "done", "blocked"] as const;
const BACKEND_OPTIONS = ["claude", "codex"] as const satisfies readonly ChatBackendKind[];
const TRIGGER_EVENT_KIND_OPTIONS = [
  "task_created",
  "task_status_changed",
  "task_updated",
  "timeline_event",
  "todo_changed",
  "interaction_request",
] as const;

function defaultWorkflowDraft() {
  const trigger = createAutomationNode("trigger", 80, 100);
  const agent = createAutomationNode("agent", 360, 100);
  return {
    nodes: [trigger, agent],
    edges: [{
      id: `${trigger.id}-${agent.id}`,
      source: trigger.id,
      target: agent.id,
    }],
    scope: { ...DEFAULT_SCOPE },
  };
}

function createAutomationNode(kind: AutomationNodeKind, x = 160, y = 160): AutomationNode {
  const id = `${kind}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
  const config: Record<string, unknown> = {};
  if (kind === "trigger") config.triggerKind = "manual";
  if (kind === "agent") {
    config.backend = "claude";
    config.model = "claude-sonnet-4-6";
    config.permission = "ask";
    config.prompt = "请根据当前上下文继续推进。";
  }
  if (kind === "tool") config.action = "record_timeline";
  if (kind === "logic") {
    config.logic = "condition";
    config.path = "trigger.kind";
  }
  if (kind === "human") config.prompt = "确认后继续执行自动化。";
  return {
    id,
    kind,
    title: NODE_KIND_LABELS[kind],
    position: { x, y },
    config,
  };
}

function nodeToFlowNode(node: AutomationNode): FlowNode {
  return {
    id: node.id,
    type: "automation",
    position: node.position,
    data: {
      node,
      selected: node.id === selectedNodeId.value,
      status: nodeStatus(node.id),
    },
  };
}

function edgeToFlowEdge(edge: AutomationEdge): FlowEdge {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    sourceHandle: edge.sourceHandle ?? undefined,
    targetHandle: edge.targetHandle ?? undefined,
    animated: false,
  };
}

function sourceHandlesForNode(node: AutomationNode): Array<{ id: string; label: string; top: string }> {
  if (node.kind !== "logic") return [{ id: "success", label: "out", top: "50%" }];
  const logic = typeof node.config.logic === "string" ? node.config.logic : "condition";
  if (logic === "condition") {
    return [
      { id: "true", label: "true", top: "38%" },
      { id: "false", label: "false", top: "66%" },
    ];
  }
  if (logic === "switch") {
    const cases = typeof node.config.cases === "string"
      ? node.config.cases.split(",").map((item) => item.trim()).filter(Boolean).slice(0, 3)
      : [];
    const handles = cases.map((item, index) => ({
      id: normalizeHandle(item),
      label: item,
      top: `${32 + index * 18}%`,
    }));
    return [...handles, { id: "default", label: "else", top: `${32 + handles.length * 18}%` }];
  }
  return [{ id: "success", label: "out", top: "50%" }];
}

function normalizeHandle(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function flowNodeToAutomation(node: FlowNode): AutomationNode {
  const existing = node.data?.node as AutomationNode | undefined;
  return {
    ...(existing ?? createAutomationNode("tool")),
    id: node.id,
    position: {
      x: node.position.x,
      y: node.position.y,
    },
  };
}

function flowEdgeToAutomation(edge: FlowEdge): AutomationEdge {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    sourceHandle: edge.sourceHandle ?? null,
    targetHandle: edge.targetHandle ?? null,
  };
}

function applyWorkflow(workflow: AutomationWorkflow | null) {
  if (!workflow) {
    workflowName.value = "新自动化";
    const draft = defaultWorkflowDraft();
    scope.value = { ...draft.scope };
    nodes.value = draft.nodes.map(nodeToFlowNode);
    edges.value = draft.edges.map(edgeToFlowEdge);
    selectedNodeId.value = draft.nodes[0]?.id ?? null;
    return;
  }
  workflowName.value = workflow.name;
  scope.value = { ...DEFAULT_SCOPE, ...workflow.scope };
  nodes.value = workflow.draft.nodes.map(nodeToFlowNode);
  edges.value = workflow.draft.edges.map(edgeToFlowEdge);
  selectedNodeId.value = workflow.draft.nodes[0]?.id ?? null;
}

async function refreshWorkflows() {
  loading.value = true;
  errorText.value = null;
  try {
    workflowRows.value = await listAutomations();
    if (!selectedWorkflowId.value && workflowRows.value.length) {
      selectedWorkflowId.value = workflowRows.value[0].id;
    }
    applyWorkflow(selectedWorkflow.value);
    await refreshRuns();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function refreshRuns() {
  runs.value = await listAutomationRuns(selectedWorkflowId.value);
  if (!selectedRunId.value || !runs.value.some((run) => run.id === selectedRunId.value)) {
    selectedRunId.value = runs.value[0]?.id ?? null;
  }
  await refreshSelectedRun();
}

async function refreshSelectedRun() {
  selectedRunDetail.value = selectedRunId.value
    ? await getAutomationRun(selectedRunId.value)
    : null;
  if (!selectedRunNodeId.value || !selectedRunNodeStates.value.some((state) => state.nodeId === selectedRunNodeId.value)) {
    selectedRunNodeId.value = selectedRunNodeStates.value[0]?.nodeId ?? null;
  }
  nodes.value = nodes.value.map((node) => ({
    ...node,
    data: {
      ...node.data,
      selected: node.id === selectedNodeId.value,
      status: nodeStatus(node.id),
    },
  }));
}

async function refreshAfterRunEvent(run: AutomationRun) {
  await refreshRuns();
  if (selectedRunId.value === run.id) {
    await refreshSelectedRun();
  }
}

function selectWorkflow(workflow: AutomationWorkflow) {
  selectedWorkflowId.value = workflow.id;
  selectedRunId.value = null;
  applyWorkflow(workflow);
  void refreshRuns().catch((err) => {
    errorText.value = String(err);
  });
  window.setTimeout(() => {
    void fitView({ padding: 0.2 });
  }, 0);
}

function newWorkflow() {
  selectedWorkflowId.value = null;
  selectedRunId.value = null;
  selectedRunDetail.value = null;
  selectedRunNodeId.value = null;
  applyWorkflow(null);
}

async function saveDraft() {
  saving.value = true;
  errorText.value = null;
  try {
    const saved = await saveAutomationDraft({
      id: selectedWorkflowId.value,
      name: workflowName.value,
      scope: scope.value,
      nodes: nodes.value.map(flowNodeToAutomation),
      edges: edges.value.map(flowEdgeToAutomation),
    });
    selectedWorkflowId.value = saved.id;
    await refreshWorkflows();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    saving.value = false;
  }
}

async function publishCurrent() {
  if (!selectedWorkflowId.value) await saveDraft();
  if (!selectedWorkflowId.value) return;
  publishing.value = true;
  errorText.value = null;
  try {
    await publishAutomation(selectedWorkflowId.value);
    await refreshWorkflows();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    publishing.value = false;
  }
}

async function toggleEnabled() {
  const workflow = selectedWorkflow.value;
  if (!workflow) return;
  errorText.value = null;
  try {
    await setAutomationEnabled(workflow.id, !workflow.enabled);
    await refreshWorkflows();
  } catch (err) {
    errorText.value = String(err);
  }
}

async function runCurrent() {
  if (!selectedWorkflowId.value) return;
  running.value = true;
  errorText.value = null;
  try {
    const run = await runAutomationOnce(selectedWorkflowId.value);
    selectedRunId.value = run.id;
    await refreshRuns();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    running.value = false;
  }
}

async function resumeSelectedRun() {
  const runId = selectedRunId.value;
  const nodeId = selectedRunNodeState.value?.nodeId;
  if (!runId || !nodeId) return;
  resuming.value = true;
  errorText.value = null;
  try {
    const run = await resumeAutomationRun(runId, {
      nodeId,
      payload: { confirmed: true },
    });
    selectedRunId.value = run.id;
    await refreshRuns();
  } catch (err) {
    errorText.value = String(err);
  } finally {
    resuming.value = false;
  }
}

function addNode(kind: AutomationNodeKind) {
  if (kind === "trigger" && hasTriggerNode.value) return;
  const node = createAutomationNode(kind, 180 + nodes.value.length * 24, 160 + nodes.value.length * 18);
  nodes.value = [...nodes.value, nodeToFlowNode(node)];
  selectedNodeId.value = node.id;
}

function updateSelectedConfig(key: string, value: unknown) {
  const node = selectedNode.value;
  if (!node) return;
  const nextAutomationNode = {
    ...(node.data.node as AutomationNode),
    config: {
      ...((node.data.node as AutomationNode).config ?? {}),
      [key]: value,
    },
  };
  nodes.value = nodes.value.map((item) =>
    item.id === node.id
      ? { ...item, data: { ...item.data, node: nextAutomationNode } }
      : item,
  );
}

function toggleScopeList(key: "projectIds" | "taskStatuses" | "eventKinds", value: string) {
  const current = scope.value[key];
  scope.value = {
    ...scope.value,
    [key]: current.includes(value)
      ? current.filter((item) => item !== value)
      : [...current, value],
  };
}

function toggleScopeBackend(value: ChatBackendKind) {
  scope.value = {
    ...scope.value,
    backends: scope.value.backends.includes(value)
      ? scope.value.backends.filter((item) => item !== value)
      : [...scope.value.backends, value],
  };
}

function configBoolean(key: string): boolean {
  const value = (selectedNode.value?.data.node as AutomationNode | undefined)?.config?.[key];
  return value === true;
}

function updateSelectedTitle(value: string) {
  const node = selectedNode.value;
  if (!node) return;
  const nextAutomationNode = {
    ...(node.data.node as AutomationNode),
    title: value,
  };
  nodes.value = nodes.value.map((item) =>
    item.id === node.id
      ? { ...item, data: { ...item.data, node: nextAutomationNode } }
      : item,
  );
}

function onNodeClick(event: { node: FlowNode }) {
  selectedNodeId.value = event.node.id;
}

function onConnect(connection: FlowConnection) {
  if (!connection.source || !connection.target) return;
  const sourceHandle = connection.sourceHandle ?? "success";
  const id = `${connection.source}-${sourceHandle}-${connection.target}`;
  if (edges.value.some((edge) => edge.id === id)) return;
  edges.value = [
    ...edges.value,
    {
      id,
      source: connection.source,
      target: connection.target,
      sourceHandle,
      targetHandle: connection.targetHandle ?? undefined,
    },
  ];
}

function configString(key: string): string {
  const value = (selectedNode.value?.data.node as AutomationNode | undefined)?.config?.[key];
  return typeof value === "string" ? value : "";
}

function nodeStatus(nodeId: string): string | null {
  return selectedRunNodeStates.value.find((state) => state.nodeId === nodeId)?.status ?? null;
}

function selectRunNodeState(state: AutomationRunNodeState) {
  selectedRunNodeId.value = state.nodeId;
  selectedNodeId.value = state.nodeId;
}

function formatDuration(state: AutomationRunNodeState | null): string {
  if (!state?.startedAt || !state.finishedAt) return "-";
  return `${Math.max(0, state.finishedAt - state.startedAt)} ms`;
}

function formatJson(value: unknown): string {
  return JSON.stringify(value ?? null, null, 2);
}

function selectedRunHumanPrompt(): string {
  const output = selectedRunNodeState.value?.output;
  const prompt = output && typeof output.prompt === "string" ? output.prompt.trim() : "";
  return prompt || "确认后继续执行自动化。";
}

function runContextMeta(run: AutomationRun): string {
  const items = [
    run.trigger.projectId ? `项目 ${run.trigger.projectId}` : "收集箱",
    run.trigger.taskId ? `任务 ${run.trigger.taskId}` : null,
    run.trigger.backend ? `后端 ${run.trigger.backend}` : null,
    run.trigger.eventKind ? `事件 ${run.trigger.eventKind}` : null,
  ].filter(Boolean);
  return items.join(" · ");
}

function runScopeMeta(run: AutomationRun): string {
  const scopeItems = [
    run.scope.projectIds.length ? `项目 ${run.scope.projectIds.join(", ")}` : run.scope.includeInbox ? "全部项目" : "仅非收集箱",
    run.scope.taskStatuses.length ? `状态 ${run.scope.taskStatuses.join(", ")}` : null,
    run.scope.backends.length ? `后端 ${run.scope.backends.join(", ")}` : null,
    run.scope.eventKinds.length ? `事件 ${run.scope.eventKinds.join(", ")}` : null,
  ].filter(Boolean);
  return `命中 scope：${scopeItems.join(" · ")}`;
}

function runStatusClass(status: string) {
  if (status === "succeeded") return "ui-badge--ok";
  if (status === "failed") return "ui-badge--err";
  if (status === "running" || status === "waiting_user") return "ui-badge--accent";
  return "ui-badge--muted";
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

function formatTime(value: number | null): string {
  if (!value) return "-";
  return new Date(value).toLocaleString();
}

function workflowMeta(workflow: AutomationWorkflow) {
  const status = workflow.enabled ? "已启用" : "已停用";
  const published = workflow.publishedVersionId ? "已发布" : "未发布";
  return `${status} · ${published}`;
}

watch(selectedNodeId, () => {
  nodes.value = nodes.value.map((node) => ({
    ...node,
    data: {
      ...node.data,
      selected: node.id === selectedNodeId.value,
      status: nodeStatus(node.id),
    },
  }));
});

watch(selectedRunId, () => {
  void refreshSelectedRun().catch((err) => {
    errorText.value = String(err);
  });
});

onMounted(async () => {
  await ensureProjectsLoaded();
  projectRows.value = listProjects();
  await refreshWorkflows();
  const changed = await onAutomationChanged(() => {
    void refreshWorkflows();
  });
  const runListeners = await onAutomationRunUpdated((event) => {
    if (!selectedWorkflowId.value || event.run.workflowId === selectedWorkflowId.value) {
      void refreshAfterRunEvent(event.run).catch((err) => {
        errorText.value = String(err);
      });
    }
  });
  unlisteners.value = [changed, ...runListeners];
});

onBeforeUnmount(() => {
  for (const unlisten of unlisteners.value) unlisten();
});
</script>

<template>
  <section class="automations-page">
    <aside class="automations-page__sidebar" aria-label="自动化列表">
      <header class="automations-page__head">
        <h1 class="automations-page__title">自动化</h1>
        <button type="button" class="ui-button ui-icon-button" title="新建自动化" aria-label="新建自动化" @click="newWorkflow">
          <Plus :size="15" aria-hidden="true" />
        </button>
      </header>
      <div v-if="loading" class="automations-page__notice">
        <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
        读取中
      </div>
      <div v-else-if="!workflowRows.length" class="automations-page__notice">没有自动化</div>
      <div v-else class="automations-page__list ui-list">
        <button
          v-for="workflow in workflowRows"
          :key="workflow.id"
          type="button"
          class="automations-page__row ui-list-item"
          :class="{ 'is-active': workflow.id === selectedWorkflowId }"
          @click="selectWorkflow(workflow)"
        >
          <span class="automations-page__row-title">{{ workflow.name }}</span>
          <span class="automations-page__row-meta">{{ workflowMeta(workflow) }}</span>
        </button>
      </div>
    </aside>

    <main class="automations-page__main">
      <header class="automations-page__toolbar">
        <input
          v-model="workflowName"
          class="automations-page__name"
          aria-label="自动化名称"
          placeholder="自动化名称"
        />
        <div class="automations-page__toolbar-actions">
          <button type="button" class="ui-button ui-button--ghost" :disabled="saving" @click="saveDraft">
            <Save :size="14" aria-hidden="true" />
            <span>{{ saving ? "保存中" : "保存草稿" }}</span>
          </button>
          <button type="button" class="ui-button ui-button--ghost" :disabled="publishing" @click="publishCurrent">
            <Check :size="14" aria-hidden="true" />
            <span>{{ publishing ? "发布中" : "发布" }}</span>
          </button>
          <button
            type="button"
            class="ui-button ui-button--ghost"
            :disabled="!selectedWorkflow"
            @click="toggleEnabled"
          >
            <ToggleRight v-if="selectedWorkflow?.enabled" :size="14" aria-hidden="true" />
            <ToggleLeft v-else :size="14" aria-hidden="true" />
            <span>{{ selectedWorkflow?.enabled ? "停用" : "启用" }}</span>
          </button>
          <button
            type="button"
            class="ui-button ui-button--primary"
            :disabled="running || !selectedWorkflow?.publishedVersionId"
            @click="runCurrent"
          >
            <Play :size="14" aria-hidden="true" />
            <span>{{ running ? "运行中" : "手动运行" }}</span>
          </button>
        </div>
      </header>

      <div v-if="errorText" class="conn-banner conn-banner--err">
        <Braces :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">自动化操作失败</div>
          <div class="conn-banner__hint">{{ errorText }}</div>
        </div>
      </div>

      <div class="automations-page__canvas">
        <div class="automations-page__palette" aria-label="节点库">
          <button
            v-for="(label, kind) in NODE_KIND_LABELS"
            :key="kind"
            type="button"
            class="ui-button ui-icon-button"
            :disabled="kind === 'trigger' && hasTriggerNode"
            :title="label"
            :aria-label="`添加${label}`"
            @click="addNode(kind as AutomationNodeKind)"
          >
            <component :is="NODE_ICONS[kind as AutomationNodeKind]" :size="14" aria-hidden="true" />
          </button>
        </div>
        <div class="automations-page__canvas-controls" role="group" aria-label="画布控制">
          <button type="button" class="ui-button ui-icon-button" title="放大" aria-label="放大画布" @click="zoomCanvasIn">
            <Plus :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="ui-button ui-icon-button" title="缩小" aria-label="缩小画布" @click="zoomCanvasOut">
            <Minus :size="14" aria-hidden="true" />
          </button>
          <button type="button" class="ui-button ui-icon-button" title="适应视图" aria-label="适应视图" @click="fitCanvas">
            <Maximize2 :size="14" aria-hidden="true" />
          </button>
        </div>
        <div class="automations-page__minimap" role="img" aria-label="节点小地图">
          <span
            v-for="node in minimapNodes"
            :key="node.id"
            class="automations-page__minimap-node"
            :class="[
              { 'is-selected': node.selected },
              node.status ? `is-${node.status}` : '',
            ]"
            :style="{ left: node.left, top: node.top, width: node.width, height: node.height }"
          />
        </div>
        <VueFlow
          v-model:nodes="nodes"
          v-model:edges="edges"
          :default-viewport="{ x: 0, y: 0, zoom: 1 }"
          fit-view-on-init
          @node-click="onNodeClick"
          @connect="onConnect"
        >
          <template #node-automation="{ data }">
            <div
              class="automations-page__node"
              :class="{ 'is-selected': data.selected }"
            >
              <Handle type="target" :position="Position.Left" class="automations-page__node-port" />
              <div class="automations-page__node-kind">
                {{ NODE_KIND_LABELS[data.node.kind as AutomationNodeKind] }}
              </div>
              <div class="automations-page__node-title">{{ data.node.title }}</div>
              <div v-if="data.status" class="automations-page__node-meta">{{ data.status }}</div>
              <div
                v-for="handle in sourceHandlesForNode(data.node as AutomationNode)"
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
    </main>

    <aside class="automations-page__inspector" aria-label="自动化检查器">
      <header class="automations-page__inspector-head">
        <h2 class="automations-page__title">检查器</h2>
      </header>
      <div class="automations-page__inspector-body">
        <section class="automations-page__section">
          <h3 class="automations-page__section-title">作用域</h3>
          <label class="ui-switch">
            <input v-model="scope.includeInbox" type="checkbox" />
            <span>包含收集箱</span>
          </label>
          <div class="automations-page__field">
            <label>项目</label>
            <div class="automations-page__chips">
              <button
                v-for="project in projectRows"
                :key="project.id"
                type="button"
                class="automations-page__chip"
                :class="{ 'is-active': scope.projectIds.includes(project.id) }"
                @click="toggleScopeList('projectIds', project.id)"
              >
                {{ project.name }}
              </button>
            </div>
          </div>
          <div class="automations-page__field">
            <label>任务状态</label>
            <div class="automations-page__chips">
              <button
                v-for="status in TASK_STATUS_OPTIONS"
                :key="status"
                type="button"
                class="automations-page__chip"
                :class="{ 'is-active': scope.taskStatuses.includes(status) }"
                @click="toggleScopeList('taskStatuses', status)"
              >
                {{ status }}
              </button>
            </div>
          </div>
          <div class="automations-page__field">
            <label>Agent 后端</label>
            <div class="automations-page__chips">
              <button
                v-for="backend in BACKEND_OPTIONS"
                :key="backend"
                type="button"
                class="automations-page__chip"
                :class="{ 'is-active': scope.backends.includes(backend) }"
                @click="toggleScopeBackend(backend)"
              >
                {{ backend }}
              </button>
            </div>
          </div>
          <div class="automations-page__field">
            <label>事件 kind 过滤</label>
            <div class="automations-page__chips">
              <button
                v-for="eventKind in TRIGGER_EVENT_KIND_OPTIONS"
                :key="eventKind"
                type="button"
                class="automations-page__chip"
                :class="{ 'is-active': scope.eventKinds.includes(eventKind) }"
                @click="toggleScopeList('eventKinds', eventKind)"
              >
                {{ eventKind }}
              </button>
            </div>
          </div>
        </section>

        <section class="automations-page__section">
          <h3 class="automations-page__section-title">节点</h3>
          <template v-if="selectedNode">
            <div class="automations-page__field">
              <label>标题</label>
              <input
                :value="(selectedNode.data.node as AutomationNode).title"
                @input="updateSelectedTitle(($event.target as HTMLInputElement).value)"
              />
            </div>
            <template v-if="(selectedNode.data.node as AutomationNode).kind === 'trigger'">
              <div class="automations-page__field">
                <label>触发类型</label>
                <select
                  :value="configString('triggerKind') || 'manual'"
                  @change="updateSelectedConfig('triggerKind', ($event.target as HTMLSelectElement).value)"
                >
                  <option value="manual">手动触发</option>
                  <option value="task_changed">任务变化</option>
                  <option value="timeline_event">时间线事件</option>
                  <option value="todo_changed">Todo 变化</option>
                  <option value="interaction_request">Agent 交互请求</option>
                </select>
              </div>
            </template>
            <template v-else-if="(selectedNode.data.node as AutomationNode).kind === 'agent'">
              <label class="ui-switch">
                <input
                  :checked="configBoolean('createTask')"
                  type="checkbox"
                  @change="updateSelectedConfig('createTask', ($event.target as HTMLInputElement).checked)"
                />
                <span>新建任务</span>
              </label>
              <div class="automations-page__field">
                <label>Task ID</label>
                <input
                  :value="configString('taskId')"
                  placeholder="${trigger.taskId}"
                  @input="updateSelectedConfig('taskId', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>项目 ID</label>
                <input
                  :value="configString('projectId')"
                  placeholder="${trigger.projectId}"
                  @input="updateSelectedConfig('projectId', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>任务标题</label>
                <input
                  :value="configString('title')"
                  placeholder="自动化 Agent 任务"
                  @input="updateSelectedConfig('title', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>后端</label>
                <select
                  :value="configString('backend') || 'claude'"
                  @change="updateSelectedConfig('backend', ($event.target as HTMLSelectElement).value)"
                >
                  <option value="claude">Claude</option>
                  <option value="codex">Codex</option>
                </select>
              </div>
              <div class="automations-page__field">
                <label>Prompt</label>
                <textarea
                  class="ui-input ui-textarea"
                  :value="configString('prompt')"
                  @input="updateSelectedConfig('prompt', ($event.target as HTMLTextAreaElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>权限</label>
                <select
                  :value="configString('permission') || 'ask'"
                  @change="updateSelectedConfig('permission', ($event.target as HTMLSelectElement).value)"
                >
                  <option value="ask">ask</option>
                  <option value="readonly">readonly</option>
                  <option value="full">full</option>
                </select>
              </div>
            </template>
            <template v-else-if="(selectedNode.data.node as AutomationNode).kind === 'logic'">
              <div class="automations-page__field">
                <label>逻辑</label>
                <select
                  :value="configString('logic') || 'condition'"
                  @change="updateSelectedConfig('logic', ($event.target as HTMLSelectElement).value)"
                >
                  <option value="condition">条件</option>
                  <option value="switch">Switch</option>
                  <option value="stop">停止运行</option>
                </select>
              </div>
              <div class="automations-page__field">
                <label>路径</label>
                <input
                  :value="configString('path')"
                  placeholder="trigger.kind"
                  @input="updateSelectedConfig('path', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>等于</label>
                <input
                  :value="configString('equals')"
                  placeholder="task_changed"
                  @input="updateSelectedConfig('equals', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div
                v-if="configString('logic') === 'switch'"
                class="automations-page__field"
              >
                <label>分支值</label>
                <input
                  :value="configString('cases')"
                  placeholder="done, blocked"
                  @input="updateSelectedConfig('cases', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </template>
            <template v-else-if="(selectedNode.data.node as AutomationNode).kind === 'tool'">
              <div class="automations-page__field">
                <label>动作</label>
                <select
                  :value="configString('action') || 'record_timeline'"
                  @change="updateSelectedConfig('action', ($event.target as HTMLSelectElement).value)"
                >
                  <option value="record_timeline">记录运行</option>
                  <option value="create_task">创建任务</option>
                  <option value="update_task_status">更新任务状态</option>
                  <option value="add_todo">添加 Todo</option>
                  <option value="send_guide">发送引导</option>
                </select>
              </div>
              <div class="automations-page__field">
                <label>Task ID</label>
                <input
                  :value="configString('taskId')"
                  placeholder="${trigger.taskId}"
                  @input="updateSelectedConfig('taskId', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>项目 ID</label>
                <input
                  :value="configString('projectId')"
                  placeholder="${trigger.projectId}"
                  @input="updateSelectedConfig('projectId', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>标题 / Todo / 引导</label>
                <input
                  :value="configString('title') || configString('text')"
                  placeholder="自动化任务"
                  @input="updateSelectedConfig(configString('action') === 'add_todo' ? 'text' : 'title', ($event.target as HTMLInputElement).value)"
                />
              </div>
              <div class="automations-page__field">
                <label>状态</label>
                <input
                  :value="configString('status')"
                  placeholder="waiting"
                  @input="updateSelectedConfig('status', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </template>
            <template v-else-if="(selectedNode.data.node as AutomationNode).kind === 'human'">
              <div class="automations-page__field">
                <label>提示</label>
                <textarea
                  class="ui-input ui-textarea"
                  :value="configString('prompt')"
                  @input="updateSelectedConfig('prompt', ($event.target as HTMLTextAreaElement).value)"
                />
              </div>
            </template>
          </template>
          <div v-else class="automations-page__notice">选择一个节点</div>
        </section>

        <section class="automations-page__section">
          <h3 class="automations-page__section-title">运行历史</h3>
          <div v-if="!runs.length" class="automations-page__notice">没有运行记录</div>
          <div v-else class="automations-page__runs">
            <button
              v-for="run in runs"
              :key="run.id"
              type="button"
              class="automations-page__run"
              :class="{ 'is-active': run.id === selectedRunId }"
              @click="selectedRunId = run.id"
            >
              <span class="automations-page__run-main">
                <span class="automations-page__run-title">{{ run.trigger.kind }}</span>
                <span class="automations-page__run-meta">{{ formatTime(run.startedAt) }}</span>
                <span class="automations-page__run-meta">{{ runContextMeta(run) }}</span>
                <span class="automations-page__run-meta">{{ runScopeMeta(run) }}</span>
                <span v-if="run.error" class="automations-page__run-error">{{ run.error }}</span>
              </span>
              <span class="ui-badge automations-page__status" :class="runStatusClass(run.status)">
                {{ run.status }}
              </span>
            </button>
          </div>
        </section>

        <section class="automations-page__section">
          <h3 class="automations-page__section-title">节点状态</h3>
          <div v-if="!selectedRunNodeStates.length" class="automations-page__notice">选择运行记录后查看节点状态</div>
          <div v-else class="automations-page__node-states">
            <button
              v-for="state in selectedRunNodeStates"
              :key="state.id"
              type="button"
              class="automations-page__node-state"
              :class="{ 'is-active': state.nodeId === selectedRunNodeId }"
              @click="selectRunNodeState(state)"
            >
              <span>{{ state.nodeId }}</span>
              <span class="ui-badge automations-page__status" :class="runStatusClass(state.status)">
                {{ state.status }}
              </span>
            </button>
          </div>
          <div v-if="selectedRunNodeState" class="automations-page__replay">
            <section
              v-if="selectedRunNodeState.status === 'waiting_user'"
              class="composer-inline composer-inline--ask automations-page__human-ask"
              role="region"
              aria-label="自动化等待确认"
            >
              <header class="composer-inline__header">
                <span class="composer-inline__icon" aria-hidden="true">
                  <CircleHelp :size="14" />
                </span>
                <span class="composer-inline__title">自动化等待确认</span>
                <span class="composer-inline__source">Automation</span>
              </header>
              <div class="composer-inline__body">
                <div class="composer-inline__question">
                  <span class="composer-inline__chip">人工节点</span>
                  <p class="composer-inline__qtext">{{ selectedRunHumanPrompt() }}</p>
                </div>
              </div>
              <footer class="composer-inline__actions">
                <span class="composer-inline__spacer" />
                <button
                  type="button"
                  class="ui-button ui-button--primary composer-inline__btn"
                  :disabled="resuming"
                  @click="resumeSelectedRun"
                >
                  <Play :size="14" aria-hidden="true" />
                  {{ resuming ? "继续中" : "继续" }}
                </button>
              </footer>
            </section>
            <ul class="kv">
              <li>
                <span>耗时</span>
                <span>{{ formatDuration(selectedRunNodeState) }}</span>
              </li>
              <li v-if="selectedRunNodeState.error">
                <span>错误</span>
                <span>{{ selectedRunNodeState.error }}</span>
              </li>
            </ul>
            <details open>
              <summary>输入</summary>
              <pre>{{ formatJson(selectedRunNodeState.input) }}</pre>
            </details>
            <details>
              <summary>输出</summary>
              <pre>{{ formatJson(selectedRunNodeState.output) }}</pre>
            </details>
          </div>
        </section>
      </div>
    </aside>
  </section>
</template>
