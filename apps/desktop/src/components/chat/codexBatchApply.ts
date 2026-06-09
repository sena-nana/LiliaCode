import type { CodexBatchApplyWorkflow } from "@lilia/contracts";

export type CodexBatchApplyInput = Pick<
  CodexBatchApplyWorkflow,
  "sourceTurnId" | "sourceKind" | "sourceSummary"
>;
