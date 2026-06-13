import type { ChatAttachment } from "./chat";

export type LiliaIabSnapshotStatus = "captured" | "metadata_only";

export interface LiliaIabSnapshot {
  taskId: string;
  url: string;
  title: string | null;
  note: string | null;
  capturedAt: number;
  screenshotPath: string | null;
  screenshotAttachment: ChatAttachment | null;
  status: LiliaIabSnapshotStatus;
  warning: string | null;
}

export interface LiliaIabSubmitResult {
  snapshot: LiliaIabSnapshot;
  delivery: "runner" | "message";
  stdinForwarded: boolean;
}
