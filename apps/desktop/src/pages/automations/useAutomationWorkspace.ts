import { computed, ref, type Ref } from "vue";
import type { AutomationScopeFilter, AutomationWorkflow, ChatBackendKind } from "@lilia/contracts";
import {
  listAutomations,
  publishAutomation,
  saveAutomationDraft,
  setAutomationEnabled,
} from "../../services/automations";
import { DEFAULT_SCOPE } from "./constants";
import type { AutomationFlowEdge, AutomationFlowNode } from "./types";

export function useAutomationWorkspace(options: {
  selectedWorkflowId: Ref<string | null>;
  applyWorkflow: (workflow: AutomationWorkflow | null) => { scope: AutomationScopeFilter };
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
}) {
  const workflowRows = ref<AutomationWorkflow[]>([]);
  const selectedWorkflowId = options.selectedWorkflowId;
  const workflowName = ref("新自动化");
  const scope = ref<AutomationScopeFilter>({ ...DEFAULT_SCOPE });
  const loading = ref(false);
  const saving = ref(false);
  const publishing = ref(false);

  const selectedWorkflow = computed(() =>
    workflowRows.value.find((workflow) => workflow.id === selectedWorkflowId.value) ?? null,
  );

  async function refreshWorkflows() {
    loading.value = true;
    options.setError("");
    try {
      workflowRows.value = await listAutomations();
      if (!selectedWorkflowId.value && workflowRows.value.length) {
        selectedWorkflowId.value = workflowRows.value[0].id;
      }
      applySelectedWorkflow();
      void options.refreshRuns().catch((err) => {
        options.setError(String(err));
      });
    } catch (err) {
      options.setError(String(err));
    } finally {
      loading.value = false;
    }
  }

  function applySelectedWorkflow() {
    const workflow = selectedWorkflow.value;
    if (!workflow) {
      workflowName.value = "新自动化";
      const draft = options.applyWorkflow(null);
      scope.value = { ...draft.scope };
      return;
    }
    workflowName.value = workflow.name;
    scope.value = { ...DEFAULT_SCOPE, ...workflow.scope };
    options.applyWorkflow(workflow);
  }

  function selectWorkflow(workflow: AutomationWorkflow) {
    selectedWorkflowId.value = workflow.id;
    options.resetRunSelection();
    applySelectedWorkflow();
    void options.refreshRuns().catch((err) => {
      options.setError(String(err));
    });
  }

  function newWorkflow() {
    selectedWorkflowId.value = null;
    options.resetRunSelection();
    workflowName.value = "新自动化";
    const draft = options.applyWorkflow(null);
    scope.value = { ...draft.scope };
  }

  async function saveDraft() {
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
      selectedWorkflowId.value = saved.id;
      await refreshWorkflows();
    } catch (err) {
      options.setError(String(err));
    } finally {
      saving.value = false;
    }
  }

  async function publishCurrent() {
    if (!selectedWorkflowId.value) await saveDraft();
    if (!selectedWorkflowId.value) return;
    publishing.value = true;
    options.setError("");
    try {
      await publishAutomation(selectedWorkflowId.value);
      await refreshWorkflows();
    } catch (err) {
      options.setError(String(err));
    } finally {
      publishing.value = false;
    }
  }

  async function toggleEnabled() {
    const workflow = selectedWorkflow.value;
    if (!workflow) return;
    options.setError("");
    try {
      await setAutomationEnabled(workflow.id, !workflow.enabled);
      await refreshWorkflows();
    } catch (err) {
      options.setError(String(err));
    }
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
    saveDraft,
    publishCurrent,
    toggleEnabled,
    toggleScopeList,
    toggleScopeBackend,
  };
}
