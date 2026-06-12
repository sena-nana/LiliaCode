import type {
  BackendEnvStatus,
  ChatBackendKind,
  CodexAppServerStatus,
  EnvStatusReport,
  RouterMode,
} from "@lilia/contracts";

export type DiagnosticTone = "ok" | "warn" | "err" | "probing";

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

export function routeLabel(mode: RouterMode): string {
  return mode === "direct" ? "直连" : "CC-Switch";
}

export function codexRuntimeIssue(status: CodexAppServerStatus | null): string {
  const issue = status?.issues.join(" ") ||
    "Codex app-server 不满足 Lilia 所需的流式事件、工具审批和 AskUser 协议能力。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认 CC-Switch 当前选中的上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。`;
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
        hint: "未找到 codex CLI。请安装 Codex CLI：npm i -g @openai/codex，并重新打开终端或 Lilia 以刷新 PATH。",
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
  if (backend === "claude") {
    return {
      tone: "ok",
      title: "Claude 运行时可用",
      hint: "Lilia 将通过本机 Node 运行 Claude Agent SDK；如发送失败，请检查 ANTHROPIC_API_KEY 或直连密钥配置。",
    };
  }
  return {
    tone: "ok",
    title: "Codex 运行时可用",
    hint: "已找到满足 Lilia 协议要求的 Codex CLI 和 app-server。",
  };
}

export function connectionDiagnostic(
  backend: ChatBackendKind,
  status: BackendEnvStatus | null,
  routerMode: RouterMode,
  ccSwitchBaseUrl: string | null,
): ProviderDiagnostic | null {
  if (!status) {
    return {
      tone: "probing",
      title: "检查中",
      hint: "正在读取连接配置。",
    };
  }
  const label = backendLabel(backend);
  if (status.connectionMode === "cc-switch") {
    const codexTail = backend === "codex"
      ? "Codex 请求会走 /responses，请确认当前上游 provider 支持 OpenAI Responses API 与当前 Codex 模型。"
      : "请求会转发到 CC-Switch 当前选中的上游 provider。";
    return {
      tone: "ok",
      title: "CC-Switch 可达",
      hint: `本地端口可达（${status.effectiveUrl ?? ccSwitchBaseUrl ?? "-"}）。${codexTail}`,
    };
  }
  if (status.connectionMode === "direct" || status.connectionMode === "custom") {
    const urlText = status.connectionMode === "custom" ? "自定义 URL" : "官方 API";
    if (status.hasApiKey) {
      return {
        tone: "ok",
        title: `${label} 直连已配置`,
        hint: `将通过${urlText} ${status.effectiveUrl ?? DIRECT_DEFAULT_URLS[backend]} 发送请求。`,
      };
    }
    return {
      tone: "warn",
      title: `${label} 直连缺少 API key`,
      hint: `当前选择直连，但未保存 ${backend === "codex" ? "OPENAI_API_KEY" : "ANTHROPIC_API_KEY"} 或设置页密钥。请填写 API key 后保存。`,
    };
  }
  if (routerMode === "direct") {
    return {
      tone: "warn",
      title: `${label} 直连未配置`,
      hint: "当前选择直连。请填写 API key；Base URL 留空时使用官方 API。",
    };
  }
  return {
    tone: "warn",
    title: "CC-Switch 不可达",
    hint: `代理 ${ccSwitchBaseUrl ?? "-"} 不可达。请启动 CC-Switch，或切到直连并填写 API key。`,
  };
}

export function providerReady(
  backend: ChatBackendKind,
  report: EnvStatusReport | null,
  status: BackendEnvStatus | null,
): boolean {
  if (!report || !status || !report.nodeAvailable) return false;
  if (backend === "codex" && !report.codexAppServer.supportsRequiredProtocol) return false;
  if (status.connectionMode === "cc-switch") return true;
  if (status.connectionMode === "direct" || status.connectionMode === "custom") {
    return status.hasApiKey;
  }
  return false;
}
