<script setup lang="ts">
import "../../styles/pages/automations.css";
import "@vue-flow/core/dist/style.css";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
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
  AutomationNode,
  AutomationNodeKind,
  AutomationRun,
  AutomationRunNodeState,
  AutomationRunSummary,
  AutomationWorkflow,
  Project,
} from "@lilia/contracts";
import { onAutomationChanged, onAutomationRunUpdated } from "../../services/automations";
import { ensureProjectsLoaded, listProjects } from "../../services/projectsStore";
import AutomationNodeInspector from "./AutomationNodeInspector.vue";
import {
  BACKEND_OPTIONS,
  NODE_KIND_LABELS,
  TASK_STATUS_OPTIONS,
  TRIGGER_EVENT_KIND_OPTIONS,
} from "./constants";
import { useAutomationGraph } from "./useAutomationGraph";
import { useAutomationRuns } from "./useAutomationRuns";
import { useAutomationWorkspace } from "./useAutomationWorkspace";

const NODE_ICONS: Record<AutomationNodeKind, unknown> = {
  trigger: Zap,
  agent: Bot,
  logic: GitBranch,
  tool: Wrench,
  human: CircleHelp,
};

const projectRows = ref<Project[]>([]);
const selectedWorkflowId = ref<string | null>(null);
const errorText = ref<string | null>(null);
const unlisteners = ref<Array<() => void>>([]);
const { fitView, zoomIn, zoomOut } = useVueFlow();

function setError(message: string) {
  errorText.value = message || null;
}

const runsController = useAutomationRuns({
  selectedWorkflowId,
  setError,
});
const graph = useAutomationGraph({
  runNodeStates: runsController.selectedRunNodeStates,
});

async function refreshRuns() {
  await runsController.refreshRuns();
  graph.refreshNodeStatuses();
}

async function refreshAfterRunEvent(run: AutomationRun) {
  await runsController.refreshAfterRunEvent(run);
  graph.refreshNodeStatuses();
}

const workspace = useAutomationWorkspace({
  selectedWorkflowId,
  applyWorkflow: graph.applyWorkflow,
  automationNodes: graph.automationNodes,
  automationEdges: graph.automationEdges,
  refreshRuns,
  resetRunSelection: runsController.resetRunSelection,
  setError,
});

const {
  workflowRows,
  workflowName,
  scope,
  loading,
  saving,
  publishing,
  selectedWorkflow,
  refreshWorkflows,
  newWorkflow,
  saveDraft,
  publishCurrent,
  toggleEnabled,
  toggleScopeList,
  toggleScopeBackend,
} = workspace;

const {
  manualRunPayloadText,
  running,
  resuming,
  runs,
  selectedRunId,
  selectedRunNodeId,
  selectedRunNodeStates,
  selectedRunNodeState,
  refreshSelectedRun,
  runCurrent,
  resumeSelectedRun,
  selectRun,
} = runsController;

const {
  selectedNodeId,
  nodes,
  edges,
  selectedNode,
  hasTriggerNode,
  minimapNodes,
  addNode,
  updateSelectedConfig,
  updateSelectedTitle,
  onNodeClick,
  onConnect,
  configString,
  configBoolean,
  sourceHandlesForNode,
} = graph;

function selectWorkflow(workflow: Parameters<typeof workspace.selectWorkflow>[0]) {
  workspace.selectWorkflow(workflow);
  window.setTimeout(() => {
    void fitView({ padding: 0.2 });
  }, 0);
}

function selectRunNodeState(state: Parameters<typeof runsController.selectRunNodeState>[0]) {
  runsController.selectRunNodeState(state);
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

function runContextMeta(run: AutomationRunSummary): string {
  const items = [
    run.projectId ? `项目 ${run.projectId}` : "收集箱",
    run.taskId ? `任务 ${run.taskId}` : null,
    run.backend ? `后端 ${run.backend}` : null,
    run.eventKind ? `事件 ${run.eventKind}` : null,
  ].filter(Boolean);
  return items.join(" · ");
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

watch(selectedRunId, () => {
  void refreshSelectedRun().then(graph.refreshNodeStatuses).catch((err) => {
    setError(String(err));
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
        setError(String(err));
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
    <Teleport to="#automation-sidebar-host">
      <div class="automations-page__sidebar">
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
      </div>
    </Teleport>

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

      <div class="automations-page__run-config">
        <label for="automation-manual-payload">手动 Payload</label>
        <textarea
          id="automation-manual-payload"
          v-model="manualRunPayloadText"
          class="ui-input ui-textarea"
          placeholder='{"source":"manual"}'
          spellcheck="false"
        />
      </div>

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
          <AutomationNodeInspector
            :selected-node="selectedNode"
            :config-string="configString"
            :config-boolean="configBoolean"
            @update-title="updateSelectedTitle"
            @update-config="updateSelectedConfig"
          />
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
              @click="selectRun(run)"
            >
              <span class="automations-page__run-main">
                <span class="automations-page__run-title">{{ run.triggerKind }}</span>
                <span class="automations-page__run-meta">{{ formatTime(run.startedAt) }}</span>
                <span class="automations-page__run-meta">{{ runContextMeta(run) }}</span>
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
