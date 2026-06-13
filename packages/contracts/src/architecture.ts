import type { ChatBackendKind } from "./chat";

export type ProjectArchitectureNodeType =
  | "module"
  | "service"
  | "component"
  | "data"
  | "external"
  | "workflow"
  | (string & {});

export type ProjectArchitectureEdgeType =
  | "depends_on"
  | "calls"
  | "reads"
  | "writes"
  | "emits"
  | "renders"
  | "owns"
  | (string & {});

export interface ProjectArchitectureNode {
  id: string;
  label: string;
  type: ProjectArchitectureNodeType;
  summary: string;
  paths: string[];
  tags: string[];
}

export interface ProjectArchitectureEdge {
  id: string;
  from: string;
  to: string;
  type: ProjectArchitectureEdgeType;
  label: string;
  summary: string;
}

export interface ProjectArchitectureGraph {
  projectId: string;
  version: number;
  summary: string;
  nodes: ProjectArchitectureNode[];
  edges: ProjectArchitectureEdge[];
  updatedAt: number;
}

export type ProjectArchitectureChange =
  | {
      type: "upsert_node";
      node: ProjectArchitectureNode;
    }
  | {
      type: "remove_node";
      nodeId: string;
    }
  | {
      type: "upsert_edge";
      edge: ProjectArchitectureEdge;
    }
  | {
      type: "remove_edge";
      edgeId: string;
    }
  | {
      type: "set_summary";
      summary: string;
    };

export type ProjectArchitecturePermission = "ask" | "full" | "readonly";

export type ProjectArchitectureChangeStatus =
  | "proposed"
  | "pending"
  | "applied"
  | "rejected"
  | "rolled_back";

export interface ProjectArchitectureChangeEvent {
  id?: string;
  projectId: string;
  taskId: string;
  turnId: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  status: ProjectArchitectureChangeStatus;
  reason: string;
  changes: ProjectArchitectureChange[];
  beforeVersion: number;
  afterVersion: number | null;
  createdAt?: number;
  resolvedAt?: number | null;
}

export interface ProjectArchitectureApplyInput {
  projectId: string;
  taskId: string;
  turnId?: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  reason: string;
  changes: ProjectArchitectureChange[];
  requestId?: string | null;
}

export interface ProjectArchitectureApplyResult {
  graph: ProjectArchitectureGraph;
  event: ProjectArchitectureChangeEvent;
}

export interface ProjectArchitectureRejectInput {
  projectId: string;
  taskId: string;
  turnId?: string | null;
  backend: ChatBackendKind;
  permission: ProjectArchitecturePermission;
  reason: string;
  changes: ProjectArchitectureChange[];
  requestId?: string | null;
}

export interface ProjectArchitectureRollbackResult {
  graph: ProjectArchitectureGraph;
  event: ProjectArchitectureChangeEvent | null;
}

export interface ProjectArchitectureChangeRecord extends ProjectArchitectureChangeEvent {
  beforeGraph?: ProjectArchitectureGraph | null;
  afterGraph?: ProjectArchitectureGraph | null;
}

export interface ProjectArchitecturePlanImpact {
  reason: string;
  changes: ProjectArchitectureChange[];
}

export interface ProjectArchitectureInteractionPayload extends ProjectArchitectureApplyInput {
  status: "pending" | "proposed" | "applied";
  requiresConfirmation: boolean;
}

export interface ProjectArchitectureInteractionResult {
  decision: "allow" | "deny";
  graph?: ProjectArchitectureGraph | null;
  event?: ProjectArchitectureChangeEvent | null;
  message?: string | null;
}
