export interface Project {
  id: string;
  name: string;
  cwd: string | null;
  sessionCount: number;
  pinned: boolean;
}

export type SessionKind = "interactive" | "headless" | "unknown";

export interface Session {
  sessionId: string;
  projectId: string;
  cwd: string;
  startedAt: number;
  kind: SessionKind;
  alive: boolean;
}

export interface ProjectSettings {
  cloneParentDir: string | null;
}

export interface PopupWindowSettings {
  shortcut: string | null;
}
