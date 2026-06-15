<script setup lang="ts">
import { computed, ref } from "vue";
import { CheckCircle2, ChevronDown, ChevronRight, Circle, X } from "lucide-vue-next";
import type {
  BackendEnvStatus,
  ChatBackendKind,
  EnvStatusReport,
  RouterMode,
} from "@lilia/contracts";
import {
  backendLabel,
  connectionDiagnostic,
  providerReady,
  routeLabel,
  runtimeDiagnostic,
  type DiagnosticTone,
} from "./providerDiagnostics";

const STORAGE_KEY = "lilia.providerSetupChecklist.dismissed";

const props = defineProps<{
  backend: ChatBackendKind;
  report: EnvStatusReport | null;
  status: BackendEnvStatus | null;
  routerMode: RouterMode;
  probing: boolean;
}>();

function dismissedFromStorage(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function writeDismissed(value: boolean) {
  try {
    if (value) localStorage.setItem(STORAGE_KEY, "1");
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    /* ignore */
  }
}

const dismissed = ref(dismissedFromStorage());
const expanded = ref(!dismissed.value);

const runtime = computed(() => runtimeDiagnostic(props.backend, props.report));
const connection = computed(() =>
  connectionDiagnostic(props.backend, props.status, props.routerMode),
);

type ChecklistStep = {
  key: string;
  title: string;
  hint: string;
  tone: DiagnosticTone;
};

const steps = computed<ChecklistStep[]>(() => {
  const report = props.report;
  const status = props.status;
  const runtimeDiag = runtime.value;
  const connectionDiag = connection.value;
  const nodeReady = report?.nodeAvailable === true;
  const codexRuntimeReady = props.backend !== "codex" ||
    report?.codexAppServer.supportsRequiredProtocol === true;
  const connectionReady = providerReady(props.backend, report, status);
  const routeMode = routeLabel(props.routerMode);
  return [
    {
      key: "runtime",
      title: "运行时检查",
      hint: runtimeDiag?.hint ?? "正在读取 Node 和本地运行时状态。",
      tone: props.probing ? "probing" : (nodeReady ? "ok" : "err"),
    },
    {
      key: "cli",
      title: "CLI 检查",
      hint: props.backend === "codex"
        ? (runtimeDiag?.hint ?? "Codex 官方账号模式需要安装 codex CLI。")
        : "Claude 通过本机 Node 运行 Claude Agent SDK；发送失败时检查 ANTHROPIC_API_KEY 或 API 密钥。",
      tone: props.probing ? "probing" : (codexRuntimeReady ? "ok" : "err"),
    },
    {
      key: "route",
      title: "接入方式",
      hint: `当前 ${backendLabel(props.backend)} 使用 ${routeMode}。API 可填写官方地址、本地代理或兼容接口；Codex 官方账号会复用 CLI 登录态。`,
      tone: "ok",
    },
    {
      key: "credential",
      title: "密钥 / 账号",
      hint: connectionDiag?.hint ?? "API 模式填写 API key；Codex 官方账号模式先完成 codex login。",
      tone: props.probing ? "probing" : (connectionReady ? "ok" : "warn"),
    },
    {
      key: "app-server",
      title: "Codex app-server",
      hint: props.backend === "codex"
        ? (runtimeDiag?.hint ?? "Codex 需要 0.128.0+ app-server 协议能力。")
        : "仅使用 Claude 时不需要 Codex app-server；切换到 Codex 前再完成该项。",
      tone: props.backend === "codex"
        ? (props.probing ? "probing" : (codexRuntimeReady ? "ok" : "err"))
        : "warn",
    },
    {
      key: "first-send",
      title: "首次发送",
      hint: connectionReady && nodeReady && codexRuntimeReady
        ? "现在可以回到对话页发送第一条消息。"
        : "完成上面的失败项后重新检测，再回到对话页发送第一条消息。",
      tone: connectionReady && nodeReady && codexRuntimeReady ? "ok" : "warn",
    },
  ];
});

const summary = computed(() => {
  const errCount = steps.value.filter((step) => step.tone === "err").length;
  const warnCount = steps.value.filter((step) => step.tone === "warn").length;
  if (props.probing) return "正在检测本机环境";
  if (errCount > 0) return `${errCount} 项需要修复`;
  if (warnCount > 0) return `${warnCount} 项需要确认`;
  return "可以开始对话";
});

function dismiss() {
  dismissed.value = true;
  expanded.value = false;
  writeDismissed(true);
}

function reopen() {
  dismissed.value = false;
  expanded.value = true;
  writeDismissed(false);
}
</script>

<template>
  <div v-if="dismissed" class="setup-checklist setup-checklist--collapsed">
    <button type="button" class="inline-link setup-checklist__reopen" @click="reopen">
      显示首次启动清单
    </button>
  </div>

  <section v-else class="setup-checklist" aria-label="首次启动清单">
    <div class="setup-checklist__head">
      <button
        type="button"
        class="setup-checklist__toggle"
        :aria-expanded="expanded"
        @click="expanded = !expanded"
      >
        <ChevronDown v-if="expanded" :size="14" aria-hidden="true" />
        <ChevronRight v-else :size="14" aria-hidden="true" />
        <span>首次启动清单</span>
        <span class="setup-checklist__summary">{{ summary }}</span>
      </button>
      <button type="button" class="setup-checklist__dismiss" aria-label="收起首次启动清单" @click="dismiss">
        <X :size="14" aria-hidden="true" />
      </button>
    </div>

    <ol v-if="expanded" class="setup-checklist__steps">
      <li
        v-for="step in steps"
        :key="step.key"
        class="setup-checklist__step"
        :class="`setup-checklist__step--${step.tone}`"
      >
        <CheckCircle2 v-if="step.tone === 'ok'" :size="15" aria-hidden="true" />
        <Circle v-else :size="15" aria-hidden="true" />
        <span class="setup-checklist__step-main">
          <span class="setup-checklist__step-title">{{ step.title }}</span>
          <span class="setup-checklist__step-hint">{{ step.hint }}</span>
        </span>
      </li>
    </ol>
  </section>
</template>
