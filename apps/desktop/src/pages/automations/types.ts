import type { AutomationNode } from "@lilia/contracts";

export interface AutomationFlowNode {
  id: string;
  type: string;
  position: { x: number; y: number };
  data: {
    node: AutomationNode;
    selected: boolean;
    status: string | null;
  };
}

export interface AutomationFlowEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  animated?: boolean;
}

export interface AutomationFlowConnection {
  source: string | null;
  target: string | null;
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

