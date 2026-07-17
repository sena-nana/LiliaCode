import { invoke } from "../tauri/runtime";
import type { Router } from "vue-router";
import type { CliProjectOpenEvent } from "@lilia/contracts";
import { CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND } from "@lilia/contracts/appEventsContract.mjs";
import { ensureProjectLoaded } from "./projectsStore";

export type CliProjectOpenPayload = CliProjectOpenEvent;

export function consumePendingCliProjectOpen(): Promise<CliProjectOpenPayload | null> {
  return invoke<CliProjectOpenPayload | null>(CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND);
}

export async function openCliProject(payload: CliProjectOpenPayload, router: Router): Promise<void> {
  const projectId = payload.projectId.trim();
  if (!projectId) return;

  const project = await ensureProjectLoaded(projectId);
  if (!project) {
    console.error("[liliacode] project not found after CLI open", payload);
    return;
  }
  if (payload.taskId) {
    await router.push(`/projects/${encodeURIComponent(project.id)}/tasks/${encodeURIComponent(payload.taskId)}`);
    return;
  }
  await router.push(`/projects/${encodeURIComponent(project.id)}`);
}
