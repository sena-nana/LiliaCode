import type {
  BackendEnvStatus,
  ChatBackendKind,
  CodexAppServerStatus,
  EnvStatusReport,
  RouterMode,
} from "@lilia/contracts";

export type DiagnosticTone = "warn" | "err" | "probing";

export interface ProviderDiagnostic {
  tone: DiagnosticTone;
  title: string;
  hint: string;
}

export const DIRECT_DEFAULT_URLS: Record<ChatBackendKind, string> = {
  claude: "https://api.anthropic.com",
  codex: "https://api.openai.com/v1",
};

export function backendLabel(backend: ChatBackendKind): string {
  return backend === "codex" ? "Codex" : "Claude";
}

export function codexRuntimeIssue(status: CodexAppServerStatus | null): string {
  const issue = status?.issues.join(" ") ||
    "Codex app-server 不满足 Lilia 所需的流式事件、工具审批和 AskUser 协议能力。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认当前 API 来源支持 OpenAI Responses API 与 Codex 模型白名单。`;
  }
  if (status?.failureKind === "missingCli") {
    return `${issue} 请安装 Codex CLI：npm i -g @openai/codex，并重新打开终端或 Lilia 以刷新 PATH。`;
  }
  if (
    status?.failureKind === "appServerUnavailable" ||
    status?.failureKind === "experimentalApiUnsupported"
  ) {
    return `${issue} 请升级 Codex CLI 到 0.128.0 或更新版本后重新检测。`;
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
        title: "Codex CLI 缺失",
        hint: "未找到 codex CLI。请安装 Codex CLI：npm i -g @openai/codex，并重新打开终端或 Lilia 以刷新 PATH；使用官方账号模式前请先运行 codex login。",
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
  const label = backendLabel(backend);
  if (status.connectionMode === "codex-account") {
    return null;
  }
  if (status.connectionMode === "custom") {
    return status.hasApiKey ? null : {
      tone: "warn",
      title: `${label} 自定义 API 来源`,
      hint: `将通过 ${status.effectiveUrl ?? "-"} 发送请求，未设置密钥；仅适用于本地代理或不需要鉴权的兼容来源。`,
    };
  }
  if (status.connectionMode === "api" && status.hasApiKey) {
    return null;
  }
  if (status.connectionMode === "api") {
    return {
      tone: "warn",
      title: `${label} API 缺少 API key`,
      hint: `当前选择 API，但未保存 ${backend === "codex" ? "OPENAI_API_KEY" : "ANTHROPIC_API_KEY"} 或设置页密钥。请填写 API key 后保存。`,
    };
  }
  if (routerMode === "codex-account") {
    return {
      tone: "warn",
      title: "Codex 官方账号未就绪",
      hint: "当前选择官方账号模式。请确认 Codex CLI 可用，并在终端运行 codex login。",
    };
  }
  return {
    tone: "warn",
    title: `${label} API 未配置`,
    hint: "当前选择 API。请填写 API key；Base URL 留空时使用官方 API，也可以填写本地代理或兼容 API 地址。",
  };
}

