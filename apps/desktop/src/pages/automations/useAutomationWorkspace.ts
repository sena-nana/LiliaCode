import { computed, ref, type Ref } from "vue";
import {
  normalizeAutomationScope,
  type AutomationScopeFilter,
  type AutomationWorkflow,
  type ChatBackendKind,
} from "@lilia/contracts";
import {
  deleteAutomation,
  listAutomations,
  publishAutomation,
  saveAutomationDraft,
  setAutomationEnabled,
} from "../../services/automations";
import type { AutomationFlowEdge, AutomationFlowNode } from "./types";

export function useAutomationWorkspace(options: {
  selectedWorkflowId: Ref<string | null>;
  automationNodes: () => AutomationFlowNode["data"]["node"][];
  automationEdges: () => {
    id: AutomationFlowEdge["id"];
    source: AutomationFlowEdge["source"];
    target: AutomationFlowEdge["target"];
    sourceHandle?: string | null;
    targetHandle?: string | null;
  }[];
  refreshRuns: () => Promise<void>;
  resetRunSelection: () => void;
  setError: (message: string) => void;
  isDisposed?: () => boolean;
}) {
  const workflowRows = ref<AutomationWorkflow[]>([]);
  const selectedWorkflowId = options.selectedWorkflowId;
  const workflowName = ref("新自动化");
  const scope = ref<AutomationScopeFilter>(normalizeAutomationScope(null));
  const loading = ref(false);
  const saving = ref(false);
  const publishing = ref(false);
  let blankWorkflowCreation: Promise<AutomationWorkflow> | null = null;
  const isDisposed = () => options.isDisposed?.() === true;

  const selectedWorkflow = computed(() =>
    workflowRows.value.find((workflow) => workflow.id === selectedWorkflowId.value) ?? null,
  );

  function syncSelectedWorkflowState() {
    const workflow = selectedWorkflow.value;
    if (!workflow) {
      workflowName.value = "新自动化";
      scope.value = normalizeAutomationScope(null);
      return;
    }
    workflowName.value = workflow.name;
    scope.value = normalizeAutomationScope(workflow.scope);
  }

  async function refreshWorkflows() {
    if (isDisposed()) return;
    loading.value = true;
    options.setError("");
    try {
      const rows = await listAutomations();
      if (isDisposed()) return;
      workflowRows.value = rows.length ? rows : [await createBlankWorkflow()];
      if (isDisposed()) return;
      if (
        !selectedWorkflowId.value ||
        !workflowRows.value.some((workflow) => workflow.id === selectedWorkflowId.value)
      ) {
        selectedWorkflowId.value = workflowRows.value[0]?.id ?? null;
      }
      syncSelectedWorkflowState();
      void options.refreshRuns().catch((err) => {
        options.setError(String(err));
      });
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) loading.value = false;
    }
  }

  function selectWorkflow(workflow: AutomationWorkflow) {
    if (isDisposed()) return;
    selectedWorkflowId.value = workflow.id;
    options.resetRunSelection();
    syncSelectedWorkflowState();
    void options.refreshRuns().catch((err) => {
      options.setError(String(err));
    });
  }

  async function createBlankWorkflow() {
    if (blankWorkflowCreation) return blankWorkflowCreation;
    const blankScope = normalizeAutomationScope(null);
    blankWorkflowCreation = saveAutomationDraft({
      id: null,
      name: "未命名自动化",
      scope: blankScope,
      nodes: [],
      edges: [],
    }).finally(() => {
      blankWorkflowCreation = null;
    });
    return blankWorkflowCreation;
  }

  async function newWorkflow() {
    if (isDisposed()) return;
    saving.value = true;
    options.setError("");
    try {
      const saved = await createBlankWorkflow();
      if (isDisposed()) return;
      workflowRows.value = [saved, ...workflowRows.value.filter((workflow) => workflow.id !== saved.id)];
      selectedWorkflowId.value = saved.id;
      options.resetRunSelection();
      syncSelectedWorkflowState();
      void options.refreshRuns().catch((err) => {
        options.setError(String(err));
      });
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) saving.value = false;
    }
  }

  async function saveDraft() {
    if (isDisposed()) return;
    saving.value = true;
    options.setError("");
    try {
      const saved = await saveAutomationDraft({
        id: selectedWorkflowId.value,
        name: workflowName.value,
        scope: scope.value,
        nodes: options.automationNodes(),
        edges: options.automationEdges(),
      });
      if (isDisposed()) return;
      selectedWorkflowId.value = saved.id;
      await refreshWorkflows();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) saving.value = false;
    }
  }

  async function publishCurrent() {
    if (isDisposed()) return;
    if (!selectedWorkflowId.value) await saveDraft();
    if (isDisposed() || !selectedWorkflowId.value) return;
    publishing.value = true;
    options.setError("");
    try {
      await publishAutomation(selectedWorkflowId.value);
      await refreshWorkflows();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) publishing.value = false;
    }
  }

  async function toggleEnabled() {
    if (isDisposed()) return;
    const workflow = selectedWorkflow.value;
    if (!workflow) return;
    options.setError("");
    try {
      await setAutomationEnabled(workflow.id, !workflow.enabled);
      await refreshWorkflows();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    }
  }

  async function deleteWorkflow(id: string) {
    if (isDisposed()) return;
    options.setError("");
    try {
      await deleteAutomation(id);
      if (isDisposed()) return;
      if (selectedWorkflowId.value === id) selectedWorkflowId.value = null;
      await refreshWorkflows();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    }
  }

  function toggleScopeList(key: "projectIds" | "taskStatuses" | "eventKinds", value: string) {
    if (isDisposed()) return;
    const current = scope.value[key];
    scope.value = {
      ...scope.value,
      [key]: current.includes(value)
        ? current.filter((item) => item !== value)
        : [...current, value],
    };
  }

  function toggleScopeBackend(value: ChatBackendKind) {
    if (isDisposed()) return;
    scope.value = {
      ...scope.value,
      backends: scope.value.backends.includes(value)
        ? scope.value.backends.filter((item) => item !== value)
        : [...scope.value.backends, value],
    };
  }

  return {
    workflowRows,
    selectedWorkflowId,
    workflowName,
    scope,
    loading,
    saving,
    publishing,
    selectedWorkflow,
    refreshWorkflows,
    selectWorkflow,
    newWorkflow,
    deleteWorkflow,
    saveDraft,
    publishCurrent,
    toggleEnabled,
    toggleScopeList,
    toggleScopeBackend,
  };
}

