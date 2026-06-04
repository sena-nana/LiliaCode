import { oneLineSummary, stringOrNull } from "../utils.mjs";
import { stringifyCodexCommand } from "../toolConsentTimeline.mjs";

export function mapCodexPermission(p) {
  switch (p) {
    case "full":
      return { sandboxMode: "danger-full-access" };
    case "readonly":
      return { sandboxMode: "read-only" };
    case "ask":
    default:
      return { sandboxMode: "workspace-write" };
  }
}

export function mapCodexSandboxMode(permission) {
  return mapCodexPermission(permission).sandboxMode;
}

export function mapCodexApprovalPolicy(permission) {
  if (permission === "full") return "never";
  if (permission === "readonly") return "never";
  return "on-request";
}

export async function maybeHandleCodexApprovalRequest(server, msg, ctx) {
  const method = stringOrNull(msg.method) || "";
  const params = msg.params && typeof msg.params === "object" && !Array.isArray(msg.params)
    ? msg.params
    : {};
  const isCommandApproval =
    method === "item/commandExecution/requestApproval";
  const isFileChangeApproval =
    method === "item/fileChange/requestApproval";
  if (!method.endsWith("/requestApproval")) return false;
  const requestId =
    stringOrNull(params.approvalId) ||
    stringOrNull(params.callId) ||
    stringOrNull(params.itemId) ||
    stringOrNull(msg.id) ||
    `codex-${method}`;
  const toolName = method;
  const input = { ...params };
  const command = stringifyCodexCommand(
    params.command ||
      params.parsedCmd ||
      params.cmd ||
      params.commandActions,
  );
  const description = isCommandApproval
    ? command
    : oneLineSummary(params.grantRoot || params.reason || method);
  const payload = {
    backend: "codex",
    toolName,
    input,
    toolUseID: requestId,
    title: isCommandApproval
      ? "Codex command approval"
      : isFileChangeApproval
        ? "Codex patch approval"
        : "Codex tool approval",
    displayName: toolName,
    description,
  };
  const { id, decision, message } = await ctx.interactions.requestUserConsent(payload);
  const accepted = decision === "allow";
  server.respond(msg.id, { decision: accepted ? "accept" : "decline" });
  ctx.emitToolConsentTimeline(id, payload, accepted ? "success" : "cancelled", accepted
    ? "用户已同意此次 Codex 操作"
    : message || "用户拒绝了此次 Codex 操作");
  return true;
}
