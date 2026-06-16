import { computed, ref, type Ref } from "vue";
import type { AutomationRun, AutomationRunNodeState, AutomationRunSummary } from "@lilia/contracts";
import {
  automationRunToSummary,
  getAutomationRun,
  listAutomationRuns,
  resumeAutomationRun,
  runAutomationOnce,
  type AutomationRunDetail,
} from "../../services/automations";

export function useAutomationRuns(options: {
  selectedWorkflowId: Ref<string | null>;
  setError: (message: string) => void;
}) {
  const running = ref(false);
  const resuming = ref(false);
  const runs = ref<AutomationRunSummary[]>([]);
  const selectedRunId = ref<string | null>(null);
  const selectedRunDetail = ref<AutomationRunDetail | null>(null);
  const selectedRunNodeId = ref<string | null>(null);

  const selectedRunNodeStates = computed<AutomationRunNodeState[]>(() =>
    selectedRunDetail.value?.nodes ?? [],
  );
  const selectedRunNodeState = computed<AutomationRunNodeState | null>(() =>
    selectedRunNodeStates.value.find((state) => state.nodeId === selectedRunNodeId.value) ?? null,
  );

  async function refreshRuns() {
    runs.value = await listAutomationRuns(options.selectedWorkflowId.value);
    if (selectedRunId.value && !runs.value.some((run) => run.id === selectedRunId.value)) {
      resetRunSelection();
    }
  }

  async function refreshSelectedRun() {
    selectedRunDetail.value = selectedRunId.value
      ? await getAutomationRun(selectedRunId.value)
      : null;
    if (
      !selectedRunNodeId.value ||
      !selectedRunNodeStates.value.some((state) => state.nodeId === selectedRunNodeId.value)
    ) {
      selectedRunNodeId.value = selectedRunNodeStates.value[0]?.nodeId ?? null;
    }
  }

  async function refreshAfterRunEvent(run: AutomationRun) {
    applyRunSummary(automationRunToSummary(run));
    if (selectedRunId.value === run.id) {
      await refreshSelectedRun();
    }
  }

  async function runCurrent() {
    if (!options.selectedWorkflowId.value) return;
    running.value = true;
    options.setError("");
    try {
      const run = await runAutomationOnce(options.selectedWorkflowId.value);
      applyRunSummary(automationRunToSummary(run));
      selectedRunId.value = run.id;
      await refreshSelectedRun();
    } catch (err) {
      options.setError(String(err));
    } finally {
      running.value = false;
    }
  }

  async function resumeSelectedRun() {
    const runId = selectedRunId.value;
    const nodeId = selectedRunNodeState.value?.nodeId;
    if (!runId || !nodeId) return;
    resuming.value = true;
    options.setError("");
    try {
      const run = await resumeAutomationRun(runId, {
        nodeId,
        payload: { confirmed: true },
      });
      applyRunSummary(automationRunToSummary(run));
      selectedRunId.value = run.id;
      await refreshSelectedRun();
    } catch (err) {
      options.setError(String(err));
    } finally {
      resuming.value = false;
    }
  }

  function resetRunSelection() {
    selectedRunId.value = null;
    selectedRunDetail.value = null;
    selectedRunNodeId.value = null;
  }

  function selectRunNodeState(state: AutomationRunNodeState) {
    selectedRunNodeId.value = state.nodeId;
  }

  function selectRun(run: AutomationRunSummary) {
    if (selectedRunId.value === run.id) return;
    selectedRunId.value = run.id;
    selectedRunDetail.value = null;
    selectedRunNodeId.value = null;
  }

  function applyRunSummary(run: AutomationRunSummary) {
    if (
      options.selectedWorkflowId.value &&
      run.workflowId !== options.selectedWorkflowId.value
    ) {
      return;
    }
    const existing = runs.value.filter((item) => item.id !== run.id);
    runs.value = [run, ...existing].sort((left, right) => right.startedAt - left.startedAt);
  }

  return {
    running,
    resuming,
    runs,
    selectedRunId,
    selectedRunDetail,
    selectedRunNodeId,
    selectedRunNodeStates,
    selectedRunNodeState,
    refreshRuns,
    refreshSelectedRun,
    refreshAfterRunEvent,
    runCurrent,
    resumeSelectedRun,
    resetRunSelection,
    selectRun,
    selectRunNodeState,
  };
}
