import type { LiliaBatchApplyWorkflow } from "@lilia/contracts";

export type LiliaBatchApplyInput = Pick<
  LiliaBatchApplyWorkflow,
  "sourceTurnId" | "sourceKind" | "sourceSummary"
>;
