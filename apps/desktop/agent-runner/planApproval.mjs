import { compactLine, isRecord } from "../../../packages/contracts/src/toolUtils.mjs";

export const PLAN_APPROVAL_QUESTION_ID = "approve-plan";

export function planApprovalBackendLabel(backend = "claude") {
  return backend === "codex" ? "Codex" : "Claude";
}

export function buildPlanApprovalSpec(backend = "claude") {
  const label = planApprovalBackendLabel(backend);
  return {
    title: `确认 ${label} 计划`,
    source: `${label} Plan`,
    intent: "plan_approval",
    dismissable: true,
    questions: [
      {
        id: PLAN_APPROVAL_QUESTION_ID,
        header: "计划确认",
        question: "",
        mode: "confirm",
        confirmLabel: "按计划执行",
        cancelLabel: "先不执行",
      },
    ],
  };
}

export function isPlanApprovalAccepted(result) {
  if (!isRecord(result) || result.cancelled === true) return false;
  const answers = isRecord(result.answers) ? result.answers : {};
  const answer = answers[PLAN_APPROVAL_QUESTION_ID];
  return isRecord(answer) && answer.value === "yes";
}

export function readPlanRevisionRequest(result) {
  if (!isRecord(result) || result.cancelled === true) return "";
  const answers = isRecord(result.answers) ? result.answers : {};
  const answer = answers[PLAN_APPROVAL_QUESTION_ID];
  if (!isRecord(answer)) return "";
  if (answer.value === "yes" || answer.value === "no") return "";
  return compactLine(answer.notes, 6000);
}

export function buildPlanRevisionDenyMessage(revisionRequest) {
  const request = compactLine(revisionRequest, 6000);
  return [
    "用户要求修改计划，暂不执行当前计划。",
    request ? `修改要求：${request}` : "",
    "请根据这条修改要求调整计划，然后再次请求确认。",
  ].filter(Boolean).join("\n");
}
