import type { ChatAttachment } from "./chat";

export type CodexIabSnapshotStatus = "captured" | "metadata_only";

export interface CodexIabSnapshot {
  taskId: string;
  url: string;
  title: string | null;
  note: string | null;
  capturedAt: number;
  screenshotPath: string | null;
  screenshotAttachment: ChatAttachment | null;
  status: CodexIabSnapshotStatus;
  warning: string | null;
}

export interface CodexIabSubmitResult {
  snapshot: CodexIabSnapshot;
  delivery: "runner" | "message";
  stdinForwarded: boolean;
}
