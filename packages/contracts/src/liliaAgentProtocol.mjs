import manifest from "./lilia-agent-protocol.json" with { type: "json" };

function buildMetadataByKind(entries, layer) {
  return Object.freeze(Object.fromEntries(
    entries.map((entry) => [
      entry.kind,
      Object.freeze({ kind: entry.kind, layer, requiresPrompt: entry.requiresPrompt }),
    ]),
  ));
}

export const LILIA_AGENT_PROTOCOL_MANIFEST = Object.freeze(manifest);
export const LILIA_WORKFLOW_METADATA = buildMetadataByKind(manifest.workflow, "workflow");
export const LILIA_RUNTIME_COMMAND_METADATA = buildMetadataByKind(
  manifest.runtimeCommand,
  "runtimeCommand",
);

export function liliaWorkflowMetadata(kind) {
  return typeof kind === "string" ? LILIA_WORKFLOW_METADATA[kind] ?? null : null;
}

export function liliaRuntimeCommandMetadata(kind) {
  return typeof kind === "string" ? LILIA_RUNTIME_COMMAND_METADATA[kind] ?? null : null;
}
