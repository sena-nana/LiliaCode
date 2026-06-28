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
  isDisposed?: () => boolean;
}) {
  const running = ref(false);
  const resuming = ref(false);
  const runs = ref<AutomationRunSummary[]>([]);
  const selectedRunId = ref<string | null>(null);
  const selectedRunDetail = ref<AutomationRunDetail | null>(null);
  const selectedRunNodeId = ref<string | null>(null);
  const isDisposed = () => options.isDisposed?.() === true;

  const selectedRunNodeStates = computed<AutomationRunNodeState[]>(() =>
    selectedRunDetail.value?.nodes ?? [],
  );
  const selectedRunNodeState = computed<AutomationRunNodeState | null>(() =>
    selectedRunNodeStates.value.find((state) => state.nodeId === selectedRunNodeId.value) ?? null,
  );

  async function refreshRuns() {
    if (isDisposed()) return;
    const nextRuns = await listAutomationRuns(options.selectedWorkflowId.value);
    if (isDisposed()) return;
    runs.value = nextRuns;
    if (selectedRunId.value && !runs.value.some((run) => run.id === selectedRunId.value)) {
      resetRunSelection();
    }
  }

  async function refreshSelectedRun() {
    if (isDisposed()) return;
    const nextDetail = selectedRunId.value
      ? await getAutomationRun(selectedRunId.value)
      : null;
    if (isDisposed()) return;
    selectedRunDetail.value = nextDetail;
    if (
      !selectedRunNodeId.value ||
      !selectedRunNodeStates.value.some((state) => state.nodeId === selectedRunNodeId.value)
    ) {
      selectedRunNodeId.value = selectedRunNodeStates.value[0]?.nodeId ?? null;
    }
  }

  async function refreshAfterRunEvent(run: AutomationRun) {
    if (isDisposed()) return;
    applyRunSummary(automationRunToSummary(run));
    if (selectedRunId.value === run.id) {
      await refreshSelectedRun();
    }
  }

  async function runCurrent() {
    if (isDisposed() || !options.selectedWorkflowId.value) return;
    running.value = true;
    options.setError("");
    try {
      const run = await runAutomationOnce(options.selectedWorkflowId.value);
      if (isDisposed()) return;
      applyRunSummary(automationRunToSummary(run));
      selectedRunId.value = run.id;
      await refreshSelectedRun();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) running.value = false;
    }
  }

  async function resumeSelectedRun() {
    if (isDisposed()) return;
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
      if (isDisposed()) return;
      applyRunSummary(automationRunToSummary(run));
      selectedRunId.value = run.id;
      await refreshSelectedRun();
    } catch (err) {
      if (!isDisposed()) options.setError(String(err));
    } finally {
      if (!isDisposed()) resuming.value = false;
    }
  }

  function resetRunSelection() {
    if (isDisposed()) return;
    selectedRunId.value = null;
    selectedRunDetail.value = null;
    selectedRunNodeId.value = null;
  }

  function selectRunNodeState(state: AutomationRunNodeState) {
    if (isDisposed()) return;
    selectedRunNodeId.value = state.nodeId;
  }

  function selectRun(run: AutomationRunSummary) {
    if (isDisposed()) return;
    if (selectedRunId.value === run.id) return;
    selectedRunId.value = run.id;
    selectedRunDetail.value = null;
    selectedRunNodeId.value = null;
  }

  function applyRunSummary(run: AutomationRunSummary) {
    if (isDisposed()) return;
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

