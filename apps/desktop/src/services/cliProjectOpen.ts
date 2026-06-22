import { invoke } from "@tauri-apps/api/core";
import type { Router } from "vue-router";
import type { CliProjectOpenEvent } from "@lilia/contracts";
import { CLI_PROJECT_OPEN_CONSUME_PENDING_COMMAND } from "@lilia/contracts";
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
  await router.push(`/projects/${encodeURIComponent(project.id)}`);
}
