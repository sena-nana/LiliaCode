import { computed, shallowRef, watch, type ComputedRef } from "vue";
import type {
  AutomationEdge,
  AutomationNode,
  AutomationNodeKind,
  AutomationRunNodeState,
  AutomationWorkflow,
} from "@lilia/contracts";
import { DEFAULT_SCOPE, NODE_KIND_LABELS } from "./constants";
import type {
  AutomationFlowConnection,
  AutomationFlowEdge,
  AutomationFlowNode,
} from "./types";

export function useAutomationGraph(options: {
  runNodeStates: ComputedRef<AutomationRunNodeState[]>;
}) {
  const selectedNodeId = shallowRef<string | null>(null);
  const nodes = shallowRef<AutomationFlowNode[]>([]);
  const edges = shallowRef<AutomationFlowEdge[]>([]);

  const selectedNode = computed(() =>
    nodes.value.find((node) => node.id === selectedNodeId.value) ?? null,
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

  function nodeToFlowNode(node: AutomationNode): AutomationFlowNode {
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

  function edgeToFlowEdge(edge: AutomationEdge): AutomationFlowEdge {
    return {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      sourceHandle: edge.sourceHandle ?? undefined,
      targetHandle: edge.targetHandle ?? undefined,
      animated: false,
    };
  }

  function sourceHandlesForNode(
    node: AutomationNode,
  ): Array<{ id: string; label: string; top: string }> {
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

  function flowNodeToAutomation(node: AutomationFlowNode): AutomationNode {
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

  function flowEdgeToAutomation(edge: AutomationFlowEdge): AutomationEdge {
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
      const draft = defaultWorkflowDraft();
      nodes.value = draft.nodes.map(nodeToFlowNode);
      edges.value = draft.edges.map(edgeToFlowEdge);
      selectedNodeId.value = draft.nodes[0]?.id ?? null;
      return draft;
    }
    nodes.value = workflow.draft.nodes.map(nodeToFlowNode);
    edges.value = workflow.draft.edges.map(edgeToFlowEdge);
    selectedNodeId.value = workflow.draft.nodes[0]?.id ?? null;
    return workflow.draft;
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
      ...node.data.node,
      config: {
        ...(node.data.node.config ?? {}),
        [key]: value,
      },
    };
    nodes.value = nodes.value.map((item) =>
      item.id === node.id
        ? { ...item, data: { ...item.data, node: nextAutomationNode } }
        : item,
    );
  }

  function updateSelectedTitle(value: string) {
    const node = selectedNode.value;
    if (!node) return;
    const nextAutomationNode = {
      ...node.data.node,
      title: value,
    };
    nodes.value = nodes.value.map((item) =>
      item.id === node.id
        ? { ...item, data: { ...item.data, node: nextAutomationNode } }
        : item,
    );
  }

  function onNodeClick(event: { node: AutomationFlowNode }) {
    selectedNodeId.value = event.node.id;
  }

  function onConnect(connection: AutomationFlowConnection) {
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
    const value = selectedNode.value?.data.node.config?.[key];
    return typeof value === "string" ? value : "";
  }

  function configBoolean(key: string): boolean {
    const value = selectedNode.value?.data.node.config?.[key];
    return value === true;
  }

  function nodeStatus(nodeId: string): string | null {
    return options.runNodeStates.value.find((state) => state.nodeId === nodeId)?.status ?? null;
  }

  function refreshNodeStatuses() {
    nodes.value = nodes.value.map((node) => ({
      ...node,
      data: {
        ...node.data,
        selected: node.id === selectedNodeId.value,
        status: nodeStatus(node.id),
      },
    }));
  }

  function automationNodes() {
    return nodes.value.map(flowNodeToAutomation);
  }

  function automationEdges() {
    return edges.value.map(flowEdgeToAutomation);
  }

  watch(selectedNodeId, refreshNodeStatuses);

  return {
    selectedNodeId,
    nodes,
    edges,
    selectedNode,
    hasTriggerNode,
    minimapNodes,
    applyWorkflow,
    addNode,
    updateSelectedConfig,
    updateSelectedTitle,
    onNodeClick,
    onConnect,
    configString,
    configBoolean,
    sourceHandlesForNode,
    refreshNodeStatuses,
    automationNodes,
    automationEdges,
  };
}

function normalizeHandle(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "");
}
