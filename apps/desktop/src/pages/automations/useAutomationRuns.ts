import { computed, ref, type Ref } from "vue";
import type { AutomationRun, AutomationRunNodeState } from "@lilia/contracts";
import {
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
  const manualRunPayloadText = ref("");
  const running = ref(false);
  const resuming = ref(false);
  const runs = ref<AutomationRun[]>([]);
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
    if (!selectedRunId.value || !runs.value.some((run) => run.id === selectedRunId.value)) {
      selectedRunId.value = runs.value[0]?.id ?? null;
    }
    await refreshSelectedRun();
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
    await refreshRuns();
    if (selectedRunId.value === run.id) {
      await refreshSelectedRun();
    }
  }

  async function runCurrent() {
    if (!options.selectedWorkflowId.value) return;
    running.value = true;
    options.setError("");
    try {
      const payload = parseManualRunPayload();
      const run = await runAutomationOnce(
        options.selectedWorkflowId.value,
        payload ? { payload } : {},
      );
      selectedRunId.value = run.id;
      await refreshRuns();
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
      selectedRunId.value = run.id;
      await refreshRuns();
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

  function parseManualRunPayload(): Record<string, unknown> | undefined {
    const payloadText = manualRunPayloadText.value.trim();
    if (!payloadText) return undefined;
    const payload = JSON.parse(payloadText) as unknown;
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
      throw new Error("手动 Payload 必须是 JSON object");
    }
    return payload as Record<string, unknown>;
  }

  return {
    manualRunPayloadText,
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
    selectRunNodeState,
  };
}
