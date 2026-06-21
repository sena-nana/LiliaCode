import {
  chatBackendLabel,
  type ChatBackendKind,
} from "./chat";
import type {
  BackendEnvStatus,
  CodexAppServerStatus,
  EnvStatusReport,
  RouterMode,
} from "./provider";
import {
  apiKeyEnvForBackend,
  connectionModeUsesCodexAccount,
  connectionModeUsesCustomUrl,
  connectionModeUsesDefaultApi,
  routerModeUsesCodexAccount,
} from "./provider";

export { DIRECT_DEFAULT_URLS } from "./provider";

export type DiagnosticTone = "warn" | "err" | "probing";

export interface ProviderDiagnostic {
  tone: DiagnosticTone;
  title: string;
  hint: string;
}

export function codexRuntimeIssue(status: CodexAppServerStatus | null): string {
  const issue = status?.issues.join(" ") ||
    "Codex app-server 不满足 Lilia 所需的流式事件、工具审批和 AskUser 协议能力。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认当前 API 来源支持 OpenAI Responses API 与 Codex 模型白名单。`;
  }
  if (status?.failureKind === "missingCli") {
    return `${issue} 请在 Provider 设置中安装或更新 Lilia 内置 Codex app-server。`;
  }
  if (
    status?.failureKind === "appServerUnavailable" ||
    status?.failureKind === "experimentalApiUnsupported"
  ) {
    return `${issue} 请在 Provider 设置中更新 Lilia 内置 Codex app-server 后重新检测。`;
  }
  return issue;
}

export function runtimeDiagnostic(
  backend: ChatBackendKind,
  report: EnvStatusReport | null,
): ProviderDiagnostic | null {
  if (!report) {
    return {
      tone: "probing",
      title: "检查中",
      hint: "正在读取本机运行时、CLI 和连接配置。",
    };
  }
  if (!report.nodeAvailable) {
    return {
      tone: "err",
      title: "Node 运行时缺失",
      hint: "未找到 Node.js 18+。Claude Agent SDK 和本地 Agent runner 需要本机 Node 运行时；安装后重启 Lilia 或重新检测。",
    };
  }
  if (backend === "codex") {
    if (!report.codexCliAvailable) {
      return {
        tone: "err",
        title: "Codex app-server 缺失",
        hint: "未找到 Lilia 内置 Codex app-server。请在 Provider 设置中安装或更新；使用官方账号模式前请先运行 codex login。",
      };
    }
    if (!report.codexAppServer.supportsRequiredProtocol) {
      return {
        tone: "err",
        title: "Codex app-server 不可用",
        hint: codexRuntimeIssue(report.codexAppServer),
      };
    }
  }
  return null;
}

export function connectionDiagnostic(
  backend: ChatBackendKind,
  status: BackendEnvStatus | null,
  routerMode: RouterMode,
): ProviderDiagnostic | null {
  if (!status) {
    return {
      tone: "probing",
      title: "检查中",
      hint: "正在读取连接配置。",
    };
  }
  const label = chatBackendLabel(backend);
  if (connectionModeUsesCodexAccount(status.connectionMode)) {
    return null;
  }
  if (connectionModeUsesCustomUrl(status.connectionMode)) {
    return status.hasApiKey ? null : {
      tone: "warn",
      title: `${label} 自定义 API 来源`,
      hint: `将通过 ${status.effectiveUrl ?? "-"} 发送请求，未设置密钥；仅适用于本地代理或不需要鉴权的兼容来源。`,
    };
  }
  if (connectionModeUsesDefaultApi(status.connectionMode) && status.hasApiKey) {
    return null;
  }
  if (connectionModeUsesDefaultApi(status.connectionMode)) {
    return {
      tone: "warn",
      title: `${label} API 缺少 API key`,
      hint: `当前选择 API，但未保存 ${apiKeyEnvForBackend(backend)} 或设置页密钥。请填写 API key 后保存。`,
    };
  }
  if (routerModeUsesCodexAccount(routerMode)) {
    return {
      tone: "warn",
      title: "Codex 官方账号未就绪",
      hint: "当前选择官方账号模式。请确认 Lilia 内置 Codex app-server 可用，并运行 codex login。",
    };
  }
  return {
    tone: "warn",
    title: `${label} API 未配置`,
    hint: "当前选择 API。请填写 API key；Base URL 留空时使用官方 API，也可以填写本地代理或兼容 API 地址。",
  };
}
