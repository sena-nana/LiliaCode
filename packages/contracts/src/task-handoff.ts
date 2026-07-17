export const LILIA_CODE_TASK_HANDOFF_PROTOCOL = "lilia-code-task-handoff" as const;
export const LILIA_CODE_TASK_HANDOFF_VERSION = 1 as const;

export type LiliaCodeTaskHandoffKind =
  | "issue"
  | "pullRequestReview"
  | "workflowFailure"
  | "syncConflict"
  | "repository";

export interface LiliaCodeTaskHandoff {
  protocol: typeof LILIA_CODE_TASK_HANDOFF_PROTOCOL;
  version: typeof LILIA_CODE_TASK_HANDOFF_VERSION;
  id: string;
  createdAt: string;
  title: string;
  kind: LiliaCodeTaskHandoffKind;
  repository: {
    fullName: string;
    worktreePath: string;
    branch: string;
    remoteUrl?: string | null;
  };
  source: {
    application: "LiliaGithub";
    route: string;
    objectUrl?: string | null;
  };
  problem: string;
  relatedFiles: string[];
  logSummary?: string | null;
  acceptanceCriteria: string[];
  pullRequest?: {
    number: number;
    baseBranch: string;
    headBranch: string;
    baseSha?: string | null;
    headSha?: string | null;
    reviewRequirements: string[];
  } | null;
  workflow?: {
    runId: number;
    runUrl: string;
    workflowName: string;
  } | null;
}

export interface LiliaCodeTaskHandoffReceipt {
  protocol: typeof LILIA_CODE_TASK_HANDOFF_PROTOCOL;
  version: typeof LILIA_CODE_TASK_HANDOFF_VERSION;
  handoffId: string;
  status: "accepted" | "incompatible" | "failed";
  taskId?: string | null;
  projectId?: string | null;
  resultRoute?: string | null;
  error?: string | null;
  updatedAt: string;
}

export interface ImportedTaskHandoff {
  taskId: string;
  handoff: LiliaCodeTaskHandoff;
  prompt: string;
}
