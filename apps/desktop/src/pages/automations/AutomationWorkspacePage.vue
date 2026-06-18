<script setup lang="ts">
import "../../styles/pages/automations.css";
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref, type Component, watch } from "vue";
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
  Trash2,
  Wrench,
  Zap,
} from "lucide-vue-next";
import type {
  AutomationNodeKind,
  AutomationWorkflow,
  Project,
} from "@lilia/contracts";
import type { AutomationRun } from "@lilia/contracts";
import { onAutomationChanged, onAutomationRunUpdated } from "../../services/automations";
import { ensureProjectsLoaded, listProjects } from "../../services/projectsStore";
import {
  NODE_KIND_LABELS,
} from "./constants";
import { useAutomationGraph } from "./useAutomationGraph";
import { useAutomationRuns } from "./useAutomationRuns";
import { useAutomationWorkspace } from "./useAutomationWorkspace";
import {
  beginPerfStage,
  cancelIdleRun,
  measurePerfAsync,
  measurePerfSync,
  runWhenIdle,
  scheduleAfterPaint,
} from "../../utils/perf";

const AutomationInspectorPanel = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "automations.inspector.load",
    async () => (await import("./AutomationInspectorPanel.vue")).default as Component,
  ),
});

const AutomationCanvasPane = defineAsyncComponent({
  suspensible: false,
  loader: () => measurePerfAsync(
    "automations.canvas.load",
    async () => (await import("./AutomationCanvasPane.vue")).default as Component,
  ),
});

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
const confirmingDeleteWorkflowId = ref<string | null>(null);
const editingWorkflowName = ref(false);
const canvasReady = ref(false);
const inspectorReady = ref(false);
const workflowHydrating = ref(false);
const unlisteners = ref<Array<() => void>>([]);
const canvasFitRequestKey = ref(0);
let canvasMountSeq = 0;
let inspectorIdleHandle: number | null = null;
let inspectorMountSeq = 0;
let pendingRunsRefresh = true;
let workspaceDisposed = false;
let listenerInstallSeq = 0;
let workflowHydrationSeq = 0;
let pendingCanvasFitAfterHydration = false;

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
  if (!inspectorReady.value) {
    pendingRunsRefresh = true;
    return;
  }
  pendingRunsRefresh = false;
  await runsController.refreshRuns();
  graph.refreshNodeStatuses();
}

async function refreshAfterRunEvent(run: AutomationRun) {
  if (!inspectorReady.value) {
    pendingRunsRefresh = true;
    return;
  }
  await runsController.refreshAfterRunEvent(run);
  graph.refreshNodeStatuses();
}

const workspace = useAutomationWorkspace({
  selectedWorkflowId,
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
  deleteWorkflow,
  saveDraft,
  publishCurrent,
  toggleEnabled,
  toggleScopeList,
  toggleScopeBackend,
} = workspace;

const {
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
  confirmingDeleteWorkflowId.value = null;
  pendingCanvasFitAfterHydration = true;
}

async function onDeleteWorkflowClick(event: Event, workflow: AutomationWorkflow) {
  event.preventDefault();
  event.stopPropagation();
  if (confirmingDeleteWorkflowId.value !== workflow.id) {
    confirmingDeleteWorkflowId.value = workflow.id;
    return;
  }
  confirmingDeleteWorkflowId.value = null;
  await deleteWorkflow(workflow.id);
}

function onWorkflowRowLeave(workflowId: string) {
  if (confirmingDeleteWorkflowId.value === workflowId) {
    confirmingDeleteWorkflowId.value = null;
  }
}

function startWorkflowNameEdit(event: PointerEvent) {
  editingWorkflowName.value = true;
  const input = event.currentTarget as HTMLInputElement;
  input.focus();
  input.select();
}

function stopWorkflowNameEdit() {
  editingWorkflowName.value = false;
}

function selectRunNodeState(state: Parameters<typeof runsController.selectRunNodeState>[0]) {
  runsController.selectRunNodeState(state);
  selectedNodeId.value = state.nodeId;
}

function workflowMeta(workflow: AutomationWorkflow) {
  const status = workflow.enabled ? "已启用" : "已停用";
  const published = workflow.publishedVersionId ? "已发布" : "未发布";
  return `${status} · ${published}`;
}

function toggleScopeIncludeInbox() {
  scope.value = {
    ...scope.value,
    includeInbox: !scope.value.includeInbox,
  };
}

function scheduleCanvasMount() {
  if (canvasReady.value) return;
  const seq = ++canvasMountSeq;
  const stage = beginPerfStage("automations.canvas.mount");
  scheduleAfterPaint(() => {
    if (seq !== canvasMountSeq || canvasReady.value) {
      stage.end("cancelled");
      return;
    }
    canvasReady.value = true;
    stage.end("paint");
  });
}

function scheduleInspectorMount() {
  if (inspectorReady.value) return;
  const seq = ++inspectorMountSeq;
  const stage = beginPerfStage("automations.inspector.mount");
  scheduleAfterPaint(() => {
    if (seq !== inspectorMountSeq || inspectorReady.value) {
      stage.end("cancelled");
      return;
    }
    inspectorIdleHandle = runWhenIdle(() => {
      inspectorIdleHandle = null;
      if (seq !== inspectorMountSeq || inspectorReady.value) {
        stage.end("cancelled");
        return;
      }
      void measurePerfAsync(
        "automations.projects.load",
        async () => {
          await ensureProjectsLoaded();
          if (seq !== inspectorMountSeq || workspaceDisposed) return;
          projectRows.value = listProjects();
        },
        { detail: "inspector" },
      ).then(() => {
        if (seq !== inspectorMountSeq || inspectorReady.value || workspaceDisposed) {
          stage.end("cancelled");
          return;
        }
        inspectorReady.value = true;
        stage.end("idle");
        if (pendingRunsRefresh) {
          void refreshRuns().catch((err) => {
            setError(String(err));
          });
        }
        if (selectedRunId.value) {
          void refreshSelectedRun().then(graph.refreshNodeStatuses).catch((err) => {
            setError(String(err));
          });
        }
      }).catch((err) => {
        setError(String(err));
        stage.end("error");
      });
    });
  });
}

function applySelectedWorkflowGraph(detail: string) {
  measurePerfSync(
    "automations.workflow.graph.apply",
    () => {
      graph.applyWorkflow(selectedWorkflow.value);
    },
    { detail },
  );
}

function scheduleWorkflowHydration(detail: string) {
  const seq = ++workflowHydrationSeq;
  workflowHydrating.value = true;
  const stage = beginPerfStage("automations.workflow.switch", { detail });
  scheduleAfterPaint(() => {
    if (workspaceDisposed || seq !== workflowHydrationSeq) {
      if (seq === workflowHydrationSeq) workflowHydrating.value = false;
      stage.end("cancelled");
      return;
    }
    applySelectedWorkflowGraph(detail);
    if (workspaceDisposed || seq !== workflowHydrationSeq) {
      if (seq === workflowHydrationSeq) workflowHydrating.value = false;
      stage.end("cancelled");
      return;
    }
    if (pendingCanvasFitAfterHydration) {
      pendingCanvasFitAfterHydration = false;
      canvasFitRequestKey.value += 1;
    }
    workflowHydrating.value = false;
    stage.end("paint");
  });
}

function installAutomationListenersInBackground() {
  const seq = ++listenerInstallSeq;
  scheduleAfterPaint(() => {
    if (workspaceDisposed || seq !== listenerInstallSeq) return;
    void measurePerfAsync(
      "automations.listeners.install",
      async () => {
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
        if (workspaceDisposed || seq !== listenerInstallSeq) {
          changed();
          for (const unlisten of runListeners) unlisten();
          return;
        }
        unlisteners.value = [changed, ...runListeners];
      },
      { detail: "workspace" },
    ).catch((err) => {
      setError(String(err));
    });
  });
}

watch(selectedRunId, () => {
  if (!selectedRunId.value) return;
  if (!inspectorReady.value) {
    pendingRunsRefresh = true;
    return;
  }
  void refreshSelectedRun().then(graph.refreshNodeStatuses).catch((err) => {
    setError(String(err));
  });
});

watch(selectedWorkflow, (workflow, previousWorkflow) => {
  const detail = previousWorkflow
    ? workflow?.id === previousWorkflow.id ? "refresh" : "select"
    : "initial";
  scheduleWorkflowHydration(detail);
});

onMounted(async () => {
  workspaceDisposed = false;
  scheduleCanvasMount();
  scheduleInspectorMount();
  await measurePerfAsync(
    "automations.workflows.load",
    () => refreshWorkflows(),
    { detail: "mount" },
  );
  installAutomationListenersInBackground();
});

onBeforeUnmount(() => {
  workspaceDisposed = true;
  canvasMountSeq += 1;
  inspectorMountSeq += 1;
  listenerInstallSeq += 1;
  workflowHydrationSeq += 1;
  if (inspectorIdleHandle !== null) {
    cancelIdleRun(inspectorIdleHandle);
    inspectorIdleHandle = null;
  }
  for (const unlisten of unlisteners.value) unlisten();
});
</script>

<template>
  <section class="automations-page">
    <Teleport to="#automation-sidebar-host">
      <div class="automations-page__sidebar">
        <div v-if="loading" class="automations-page__notice">
          <Loader2 :size="14" class="is-spinning" aria-hidden="true" />
          读取中
        </div>
        <div v-else class="automations-page__list sb-tree">
          <button
            v-for="workflow in workflowRows"
            :key="workflow.id"
            type="button"
            class="automations-page__row sb-tree__row sb-tree__row--unified"
            :class="{ 'is-active': workflow.id === selectedWorkflowId }"
            :title="workflow.name"
            @click="selectWorkflow(workflow)"
            @mouseleave="onWorkflowRowLeave(workflow.id)"
          >
            <span class="automations-page__row-title sb-tree__name">{{ workflow.name }}</span>
            <span class="automations-page__row-meta sb-tree__project-label">{{ workflowMeta(workflow) }}</span>
            <span class="sb-tree__hover-tools" @click.stop>
              <button
                type="button"
                class="sb-icon-btn"
                :class="{ 'is-confirming': confirmingDeleteWorkflowId === workflow.id }"
                :title="confirmingDeleteWorkflowId === workflow.id ? '确认删除，再点一次' : '删除'"
                :aria-label="confirmingDeleteWorkflowId === workflow.id ? '确认删除' : '删除'"
                @click="onDeleteWorkflowClick($event, workflow)"
              >
                <template v-if="confirmingDeleteWorkflowId === workflow.id">确认</template>
                <Trash2 v-else :size="13" aria-hidden="true" />
              </button>
            </span>
          </button>
        </div>
      </div>
    </Teleport>
    <Teleport to="#automation-sidebar-actions">
      <button
        type="button"
        class="ui-button ui-icon-button"
        title="新建自动化"
        aria-label="新建自动化"
        :disabled="saving"
        @click="newWorkflow"
      >
        <Plus :size="14" aria-hidden="true" />
      </button>
    </Teleport>

    <main class="automations-page__main">
      <AutomationCanvasPane
        v-if="canvasReady"
        v-model:nodes="nodes"
        v-model:edges="edges"
        :fit-request-key="canvasFitRequestKey"
        :source-handles-for-node="sourceHandlesForNode"
        :on-node-click="onNodeClick"
        :on-connect="onConnect"
      >
        <template #default="{ fitCanvas, zoomCanvasIn, zoomCanvasOut }">
          <div class="automations-page__corner-actions automations-page__palette" aria-label="节点库">
            <button
              v-for="(label, kind) in NODE_KIND_LABELS"
              :key="kind"
              type="button"
              class="ui-button ui-icon-button"
              :disabled="workflowHydrating || (kind === 'trigger' && hasTriggerNode)"
              :title="label"
              :aria-label="`添加${label}`"
              @click="addNode(kind as AutomationNodeKind)"
            >
              <component :is="NODE_ICONS[kind as AutomationNodeKind]" :size="14" aria-hidden="true" />
            </button>
          </div>
          <div class="automations-page__corner-actions automations-page__workflow-actions" role="group" aria-label="自动化操作">
            <input
              v-model="workflowName"
              class="automations-page__name"
              :class="{ 'is-editing': editingWorkflowName }"
              aria-label="自动化名称"
              placeholder="自动化名称"
              :readonly="!editingWorkflowName"
              @pointerdown="startWorkflowNameEdit"
              @blur="stopWorkflowNameEdit"
              @keydown.enter.prevent="stopWorkflowNameEdit"
              @keydown.esc.prevent="stopWorkflowNameEdit"
            />
            <button
              type="button"
              class="ui-button ui-icon-button"
              :title="saving ? '保存中' : '保存草稿'"
              :aria-label="saving ? '保存中' : '保存草稿'"
              :disabled="saving || workflowHydrating"
              @click="saveDraft"
            >
              <Save :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="ui-button ui-icon-button"
              :title="publishing ? '发布中' : '发布'"
              :aria-label="publishing ? '发布中' : '发布'"
              :disabled="publishing || workflowHydrating"
              @click="publishCurrent"
            >
              <Check :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="ui-button ui-icon-button"
              :title="selectedWorkflow?.enabled ? '停用' : '启用'"
              :aria-label="selectedWorkflow?.enabled ? '停用' : '启用'"
              :disabled="!selectedWorkflow || workflowHydrating"
              @click="toggleEnabled"
            >
              <ToggleRight v-if="selectedWorkflow?.enabled" :size="14" aria-hidden="true" />
              <ToggleLeft v-else :size="14" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="ui-button ui-icon-button ui-button--primary"
              :title="running ? '运行中' : '手动运行'"
              :aria-label="running ? '运行中' : '手动运行'"
              :disabled="running || workflowHydrating || !selectedWorkflow?.publishedVersionId"
              @click="runCurrent"
            >
              <Play :size="14" aria-hidden="true" />
            </button>
          </div>
          <div class="automations-page__corner-actions automations-page__canvas-controls" role="group" aria-label="画布控制">
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
          <div v-if="errorText" class="conn-banner conn-banner--err automations-page__canvas-error">
            <Braces :size="16" aria-hidden="true" />
            <div>
              <div class="conn-banner__title">自动化操作失败</div>
              <div class="conn-banner__hint">{{ errorText }}</div>
            </div>
          </div>
        </template>
      </AutomationCanvasPane>
      <section
        v-else
        class="automations-page__canvas"
        aria-busy="true"
        aria-label="自动化画布加载中"
      >
        <div class="automations-page__inspector-body">
          <section class="automations-page__section">
            <div class="automations-page__notice">正在准备自动化画布…</div>
          </section>
        </div>
      </section>
    </main>

    <aside class="automations-page__inspector" :aria-busy="!inspectorReady" aria-label="自动化检查器">
      <header class="automations-page__inspector-head">
        <h2 class="automations-page__title">检查器</h2>
      </header>
      <AutomationInspectorPanel
        v-if="inspectorReady"
        :scope="scope"
        :project-rows="projectRows"
        :selected-node="selectedNode"
        :config-string="configString"
        :config-boolean="configBoolean"
        :runs="runs"
        :selected-run-id="selectedRunId"
        :selected-run-node-id="selectedRunNodeId"
        :selected-run-node-states="selectedRunNodeStates"
        :selected-run-node-state="selectedRunNodeState"
        :resuming="resuming"
        @toggle-scope-include-inbox="toggleScopeIncludeInbox"
        @toggle-scope-list="toggleScopeList"
        @toggle-scope-backend="toggleScopeBackend"
        @update-title="updateSelectedTitle"
        @update-config="updateSelectedConfig"
        @select-run="selectRun"
        @select-run-node-state="selectRunNodeState"
        @resume-selected-run="resumeSelectedRun"
      />
      <div v-else class="automations-page__inspector-body">
        <section class="automations-page__section">
          <div class="automations-page__notice">首屏加载后补齐检查器与运行历史…</div>
        </section>
      </div>
    </aside>
  </section>
</template>
