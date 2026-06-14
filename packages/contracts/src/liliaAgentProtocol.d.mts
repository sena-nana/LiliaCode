export interface LiliaProtocolKindMetadata {
  kind: string;
  layer: "workflow" | "runtimeCommand";
  requiresPrompt: boolean;
}

export interface LiliaAgentProtocolManifest {
  workflow: ReadonlyArray<Omit<LiliaProtocolKindMetadata, "layer">>;
  runtimeCommand: ReadonlyArray<Omit<LiliaProtocolKindMetadata, "layer">>;
}

export const LILIA_AGENT_PROTOCOL_MANIFEST: Readonly<LiliaAgentProtocolManifest>;
export const LILIA_WORKFLOW_METADATA: Readonly<Record<string, LiliaProtocolKindMetadata>>;
export const LILIA_RUNTIME_COMMAND_METADATA: Readonly<Record<string, LiliaProtocolKindMetadata>>;

export function liliaWorkflowMetadata(kind: unknown): LiliaProtocolKindMetadata | null;
export function liliaRuntimeCommandMetadata(kind: unknown): LiliaProtocolKindMetadata | null;
