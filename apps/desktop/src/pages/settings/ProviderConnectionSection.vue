<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { AlertTriangle, Loader2, Network, Plug, Save } from "lucide-vue-next";
import type { CCSwitchConfig, ChatBackendKind } from "@lilia/contracts";
import { useConnectionStatus } from "../../composables/useConnectionStatus";
import {
  getCCSwitchConfig,
  setCCSwitchConfig,
  setRouterMode,
} from "../../services/chat";

const {
  activeBackend,
  setActiveBackend,
  statusFor,
  probing,
  refresh,
  nodeAvailable,
  codexCliAvailable,
  codexAppServer,
  ccSwitch,
} = useConnectionStatus();

const backendOptions: { value: ChatBackendKind; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
];
const switchingBackend = ref<ChatBackendKind | null>(null);
const ccSwitchForm = ref<CCSwitchConfig>({ baseUrl: "http://127.0.0.1:15721" });
const savingCCSwitch = ref(false);

const selectedBackend = computed(() => activeBackend.value);
const selectedStatus = computed(() => statusFor(selectedBackend.value));
const isClaude = computed(() => selectedBackend.value === "claude");
const selectedOk = computed(() => selectedStatus.value?.connectionMode === "cc-switch");

function codexRuntimeIssueText(): string {
  const status = codexAppServer.value;
  const issue = status?.issues.join(" ") ||
    "Codex app-server 不满足 Lilia 所需的流式事件、工具审批和 AskUser 协议能力。";
  if (status?.failureKind === "providerIncompatible") {
    return `${issue} 请确认 CC-Switch 当前选中的上游 provider 支持 OpenAI Responses API 与 Codex 模型白名单。`;
  }
  if (status?.failureKind === "missingCli") {
    return issue;
  }
  if (status?.failureKind === "appServerUnavailable") {
    return `${issue} 请安装或升级 Codex CLI 到 0.128.0 或更新版本后重新检测。`;
  }
  if (status?.failureKind === "experimentalApiUnsupported") {
    return `${issue} 请升级 Codex CLI 到 0.128.0 或更新版本后重新检测。`;
  }
  return issue;
}

const selectedRuntimeIssue = computed<string | null>(() => {
  if (probing.value) return null;
  if (!nodeAvailable.value) return "未找到 node（v18+），SDK 需要本机 Node 运行时。";
  if (!isClaude.value && !codexCliAvailable.value) return "未找到 codex CLI。请先 npm i -g @openai/codex 再重新检测。";
  if (!isClaude.value && codexAppServer.value && !codexAppServer.value.supportsRequiredProtocol) {
    return codexRuntimeIssueText();
  }
  return null;
});

const selectedHint = computed(() => {
  const s = selectedStatus.value;
  if (!s) return "正在检测…";
  if (s.connectionMode === "cc-switch") {
    if (!isClaude.value) {
      return `CC-Switch 本地端口可达（${s.effectiveUrl ?? "—"}）。实际 Codex 请求会走 /responses，请确认当前上游 provider 支持 OpenAI Responses API 与当前 Codex 模型。`;
    }
    return `CC-Switch 本地端口可达（${s.effectiveUrl ?? "—"}），实际请求会转发到当前选中的上游 provider。`;
  }
  return `代理 ${ccSwitch.value?.baseUrl ?? "—"} 不可达。请检查 CC-Switch 是否在运行，或修改下方 URL。`;
});

async function loadConfig() {
  try { ccSwitchForm.value = await getCCSwitchConfig(); }
  catch (err) { console.error("[settings] load cc-switch config failed", err); }
}

async function lockRouters() {
  try {
    await Promise.all([
      setRouterMode("claude", "cc-switch"),
      setRouterMode("codex", "cc-switch"),
    ]);
  } catch (err) {
    console.error("[settings] lock router mode failed", err);
  }
}

async function saveCCSwitch() {
  const cfg: CCSwitchConfig = { baseUrl: ccSwitchForm.value.baseUrl?.trim() || null };
  savingCCSwitch.value = true;
  try {
    await setCCSwitchConfig(cfg);
    await refresh();
  } catch (err) { console.error("[settings] setCCSwitchConfig failed", err); }
  finally { savingCCSwitch.value = false; }
}

async function probe() { await refresh(); }

async function selectBackend(backend: ChatBackendKind) {
  if (switchingBackend.value) return;
  switchingBackend.value = backend;
  try {
    await setActiveBackend(backend);
    await refresh();
  } catch (err) {
    console.error("[settings] setActiveBackend failed", err);
  } finally {
    switchingBackend.value = null;
  }
}

onMounted(async () => {
  await lockRouters();
  await Promise.all([loadConfig(), refresh()]);
});
</script>

<template>
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Network :size="14" aria-hidden="true" />
        连接
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">使用</div>
      <div class="segmented" role="radiogroup" aria-label="Backend">
        <button
          v-for="opt in backendOptions"
          :key="opt.value"
          type="button"
          role="radio"
          :aria-checked="selectedBackend === opt.value"
          :class="{ 'is-active': selectedBackend === opt.value }"
          :disabled="switchingBackend !== null"
          @click="selectBackend(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">代理 URL</div>
      <div style="display: flex; gap: 8px; align-items: center;">
        <input
          type="text"
          class="text-input"
          placeholder="http://127.0.0.1:15721"
          :value="ccSwitchForm.baseUrl ?? ''"
          @input="(e) => (ccSwitchForm.baseUrl = (e.target as HTMLInputElement).value)"
        />
        <button type="button" class="ghost" :disabled="savingCCSwitch" @click="saveCCSwitch">
          <Save :size="12" aria-hidden="true" />
          {{ savingCCSwitch ? "保存中…" : "保存" }}
        </button>
      </div>
    </div>

    <div v-if="probing" class="conn-banner conn-banner--probing">
      <Loader2 :size="14" class="status-pill__spin" aria-hidden="true" />
      <div><div class="conn-banner__title">检查中…</div></div>
    </div>
    <div v-else-if="selectedRuntimeIssue" class="conn-banner conn-banner--err">
      <AlertTriangle :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">{{ isClaude ? "Claude" : "Codex" }} 运行环境不满足</div>
        <div class="conn-banner__hint">
          {{ selectedRuntimeIssue }}
          <button type="button" class="inline-link" :disabled="probing" @click="probe">重新检测</button>
        </div>
      </div>
    </div>
    <div
      v-else-if="selectedStatus"
      class="conn-banner"
      :class="selectedOk ? 'conn-banner--ok' : 'conn-banner--err'"
    >
      <component :is="selectedOk ? Plug : AlertTriangle" :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">{{ selectedOk ? "代理可达" : "未连接" }}</div>
        <div class="conn-banner__hint">{{ selectedHint }}</div>
      </div>
    </div>
  </div>
</template>
