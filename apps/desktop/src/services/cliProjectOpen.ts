import { invoke } from "@tauri-apps/api/core";
import type { Router } from "vue-router";
import { ensureProjectLoaded } from "./projectsStore";

export interface CliProjectOpenPayload {
  projectId: string;
  cwd: string;
}

export function consumePendingCliProjectOpen(): Promise<CliProjectOpenPayload | null> {
  return invoke<CliProjectOpenPayload | null>("cli_project_open_consume_pending");
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
