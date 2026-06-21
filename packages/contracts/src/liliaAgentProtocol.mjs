import {
  AUTOMATION_WORKFLOW_TYPE,
  CHAT_SLASH_COMMAND_WORKFLOW_TYPE,
  LILIA_BACKGROUND_TERMINALS_CLEAN_WORKFLOW_TYPE,
  LILIA_BATCH_APPLY_WORKFLOW_TYPE,
  LILIA_COMPACT_WORKFLOW_TYPE,
  LILIA_CONFIG_DIAGNOSTICS_WORKFLOW_TYPE,
  LILIA_FIX_SUGGESTION_WORKFLOW_TYPE,
  LILIA_GOAL_WORKFLOW_TYPE,
  LILIA_MEMORY_MODE_WORKFLOW_TYPE,
  LILIA_MEMORY_RESET_WORKFLOW_TYPE,
  LILIA_REVIEW_WORKFLOW_TYPE,
} from "./liliaWorkflowContract.mjs";
import {
  REMOTE_ENVIRONMENT_COMMAND_TYPE,
  RUNTIME_SETTINGS_COMMAND_TYPE,
  SANDBOX_DIAGNOSTICS_COMMAND_TYPE,
  SESSION_FORK_COMMAND_TYPE,
} from "./runtimeCommandContract.mjs";
import { SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE } from "./sessionManagementContract.mjs";

function metadataEntry(kind) {
  return Object.freeze({ kind, requiresPrompt: false });
}

function buildManifest() {
  return Object.freeze({
    workflow: Object.freeze([
      metadataEntry(LILIA_REVIEW_WORKFLOW_TYPE),
      metadataEntry(LILIA_FIX_SUGGESTION_WORKFLOW_TYPE),
      metadataEntry(LILIA_BATCH_APPLY_WORKFLOW_TYPE),
      metadataEntry(LILIA_GOAL_WORKFLOW_TYPE),
      metadataEntry(LILIA_COMPACT_WORKFLOW_TYPE),
      metadataEntry(LILIA_BACKGROUND_TERMINALS_CLEAN_WORKFLOW_TYPE),
      metadataEntry(LILIA_MEMORY_MODE_WORKFLOW_TYPE),
      metadataEntry(LILIA_MEMORY_RESET_WORKFLOW_TYPE),
      metadataEntry(LILIA_CONFIG_DIAGNOSTICS_WORKFLOW_TYPE),
      metadataEntry(AUTOMATION_WORKFLOW_TYPE),
      metadataEntry(CHAT_SLASH_COMMAND_WORKFLOW_TYPE),
    ]),
    runtimeCommand: Object.freeze([
      metadataEntry(SESSION_FORK_COMMAND_TYPE),
      metadataEntry(SESSION_MANAGEMENT_RUNTIME_COMMAND_TYPE),
      metadataEntry(RUNTIME_SETTINGS_COMMAND_TYPE),
      metadataEntry(REMOTE_ENVIRONMENT_COMMAND_TYPE),
      metadataEntry(SANDBOX_DIAGNOSTICS_COMMAND_TYPE),
    ]),
  });
}

function buildMetadataByKind(entries, layer) {
  return Object.freeze(Object.fromEntries(
    entries.map((entry) => [
      entry.kind,
      Object.freeze({ kind: entry.kind, layer, requiresPrompt: entry.requiresPrompt }),
    ]),
  ));
}

const manifest = buildManifest();

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
