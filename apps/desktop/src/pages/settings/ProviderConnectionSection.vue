<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  AlertTriangle,
  Download,
  KeyRound,
  LogIn,
  Loader2,
  Network,
  RotateCw,
  Save,
  Trash2,
  UserRound,
} from "lucide-vue-next";
import {
  API_KEY_ENV_BY_BACKEND,
  CHAT_BACKENDS,
  DEFAULT_ROUTER_MODE_BY_BACKEND,
  DIRECT_DEFAULT_URLS,
  apiDescriptionForBackend,
  chatBackendLabel,
  connectionDiagnostic,
  createChatBackendRecord,
  defaultRouterModeForBackend,
  normalizeRouterModeForBackend,
  routerModeLabel,
  routerModeUsesApiConfig,
  routerModesForBackend,
  runtimeDiagnostic,
  type CodexAccountQuotaStatus,
  type ChatBackendKind,
  type ProviderConfig,
  type RouterMode,
} from "@lilia/contracts";
import { useConnectionStatus } from "../../composables/useConnectionStatus";
import {
  getProviderConfig,
  getCodexAccountQuotaStatus,
  getRouterMode,
  setProviderConfig,
  setRouterMode,
  startCodexAccountLogin,
} from "../../services/chat";
import { codexQuotaUnavailableStatus } from "../../utils/quotaDisplay";
import RemoteControlSection from "./RemoteControlSection.vue";

const {
  report,
  activeBackend,
  setActiveBackend,
  statusFor,
  probing,
  refresh,
  checkCodexAppServerUpdate,
  installCodexAppServerUpdate,
  codexAppServerUpdating,
  codexAppServerUpdateChecking,
  codexAppServerUpdateError,
} = useConnectionStatus();

const backendOptions: { value: ChatBackendKind; label: string }[] = CHAT_BACKENDS.map((backend) => ({
  value: backend,
  label: chatBackendLabel(backend),
}));
const codexRouterModes = [
  DEFAULT_ROUTER_MODE_BY_BACKEND.codex,
  ...routerModesForBackend("codex").filter((mode) => mode !== DEFAULT_ROUTER_MODE_BY_BACKEND.codex),
];
const codexModeOptions: { value: RouterMode; label: string }[] = codexRouterModes.map((mode) => ({
  value: mode,
  label: routerModeLabel(mode),
}));

function emptyProviderConfig(backend: ChatBackendKind): ProviderConfig {
  return { backend, baseUrl: null, apiKey: null, hasApiKey: false };
}

function providerConfigMapFromBackends(): Record<ChatBackendKind, ProviderConfig> {
  return createChatBackendRecord(emptyProviderConfig);
}

function routerModeMapFromBackends(): Record<ChatBackendKind, RouterMode> {
  return createChatBackendRecord(defaultRouterModeForBackend);
}

const switchingBackend = ref<ChatBackendKind | null>(null);
const savingProvider = ref(false);
const savingRouter = ref(false);
const providerForms = ref<Record<ChatBackendKind, ProviderConfig>>(providerConfigMapFromBackends());
const routerModes = ref<Record<ChatBackendKind, RouterMode>>(routerModeMapFromBackends());

const selectedBackend = computed(() => activeBackend.value);
const selectedStatus = computed(() => statusFor(selectedBackend.value));
const selectedRouterMode = computed(() => routerModes.value[selectedBackend.value]);
const selectedProviderForm = computed(() => providerForms.value[selectedBackend.value]);
const selectedRuntime = computed(() => runtimeDiagnostic(selectedBackend.value, report.value));
const selectedConnection = computed(() =>
  connectionDiagnostic(
    selectedBackend.value,
    selectedStatus.value,
    selectedRouterMode.value,
  ),
);
const selectedDiagnostic = computed(() => {
  if (probing.value) {
    return { tone: "probing" as const, title: "检查中", hint: "正在读取本机运行时和连接配置。" };
  }
  const runtime = selectedRuntime.value;
  if (runtime) return runtime;
  return selectedConnection.value;
});
const apiDefaultUrl = computed(() => DIRECT_DEFAULT_URLS[selectedBackend.value]);
const apiKeyEnv = computed(() => API_KEY_ENV_BY_BACKEND[selectedBackend.value]);
const apiDescription = computed(() => apiDescriptionForBackend(selectedBackend.value));
const showApiConfig = computed(() => routerModeUsesApiConfig(selectedRouterMode.value));
const codexAppServerStatus = computed(() => report.value?.codexAppServer ?? null);
const showCodexUpdateAction = computed(() =>
  selectedBackend.value === "codex" &&
  selectedRouterMode.value === "codex-account" &&
  Boolean(codexAppServerStatus.value?.updateAvailable),
);
const codexUpdateLabel = computed(() =>
  codexAppServerStatus.value?.latestVersion
    ? `更新到 ${codexAppServerStatus.value.latestVersion}`
    : "安装更新",
);
const codexVersionText = computed(() => {
  const current = codexAppServerStatus.value?.version ?? "未安装";
  const latest = codexAppServerStatus.value?.latestVersion;
  return latest ? `${current} / latest ${latest}` : current;
});
const codexInstallPathText = computed(() =>
  codexAppServerStatus.value?.installPath
    ? `路径：${codexAppServerStatus.value.installPath}`
    : "将安装到 Lilia 管理目录",
);
const codexAccountStatus = ref<CodexAccountQuotaStatus | null>(null);
const codexAccountLoading = ref(false);
const codexLoginStarting = ref(false);
let codexAccountRequestSeq = 0;
const showCodexRuntimeStatus = computed(() =>
  selectedBackend.value === "codex" && selectedRouterMode.value === "codex-account",
);
const codexRuntimeStatusText = computed(() =>
  codexAppServerStatus.value?.supportsRequiredProtocol
    ? "app-server 可用"
    : "app-server 不可用",
);
const codexLoginNeedsAction = computed(() => {
  const status = codexAccountStatus.value;
  if (!status || status.available) return false;
  const text = (status.error ?? "").toLowerCase();
  if (!text.trim()) return codexAppServerStatus.value?.supportsRequiredProtocol ?? false;
  return (
    text.includes("未登录") ||
    text.includes("not logged") ||
    text.includes("login") ||
    text.includes("auth")
  );
});
const codexLoginStatusText = computed(() => {
  if (codexAccountLoading.value) return "登录状态：检查中";
  if (codexAccountStatus.value?.available) return "登录状态：已登录";
  if (codexLoginNeedsAction.value) return "登录状态：未登录";
  if (codexAccountStatus.value) return "登录状态：无法确认";
  return "登录状态：待检测";
});
const codexLoginStatusDetail = computed(() =>
  codexAccountStatus.value?.available ? null : codexAccountStatus.value?.error,
);
let disposed = false;

async function loadProvider(backend: ChatBackendKind) {
  try {
    const config = await getProviderConfig(backend);
    if (disposed) return;
    providerForms.value = {
      ...providerForms.value,
      [backend]: { ...config, apiKey: null },
    };
  } catch (err) {
    console.error("[settings] load provider config failed", err);
  }
}

async function loadRouter(backend: ChatBackendKind) {
  try {
    const mode = await getRouterMode(backend);
    if (disposed) return;
    routerModes.value = {
      ...routerModes.value,
      [backend]: normalizeRouterModeForBackend(backend, mode),
    };
  } catch (err) {
    console.error("[settings] load router mode failed", err);
  }
}

async function loadAllConfig() {
  await Promise.all(CHAT_BACKENDS.flatMap((backend) => [
    loadProvider(backend),
    loadRouter(backend),
  ]));
}

function normalizedProviderConfig(clearApiKey = false): ProviderConfig {
  const form = selectedProviderForm.value;
  return {
    backend: selectedBackend.value,
    baseUrl: form.baseUrl?.trim() || null,
    apiKey: form.apiKey?.trim() || null,
    hasApiKey: form.hasApiKey,
    clearApiKey,
  };
}

async function saveProvider() {
  if (disposed) return;
  const backend = selectedBackend.value;
  savingProvider.value = true;
  try {
    await setProviderConfig(normalizedProviderConfig(false));
    if (disposed) return;
    await loadProvider(backend);
    if (disposed) return;
    await refresh();
  } catch (err) {
    console.error("[settings] save provider config failed", err);
  } finally {
    if (!disposed) savingProvider.value = false;
  }
}

async function clearProviderKey() {
  if (disposed) return;
  const backend = selectedBackend.value;
  savingProvider.value = true;
  try {
    await setProviderConfig({ ...normalizedProviderConfig(true), apiKey: null, clearApiKey: true });
    if (disposed) return;
    providerForms.value[backend].apiKey = null;
    await loadProvider(backend);
    if (disposed) return;
    await refresh();
  } catch (err) {
    console.error("[settings] clear provider key failed", err);
  } finally {
    if (!disposed) savingProvider.value = false;
  }
}

async function selectRouterMode(mode: RouterMode) {
  if (disposed) return;
  const backend = selectedBackend.value;
  if (
    savingRouter.value ||
    routerModes.value[backend] === mode ||
    normalizeRouterModeForBackend(backend, mode) !== mode
  ) {
    return;
  }
  const previous = routerModes.value[backend];
  routerModes.value = { ...routerModes.value, [backend]: mode };
  savingRouter.value = true;
  try {
    await setRouterMode(backend, mode);
    if (disposed) return;
    await refresh();
  } catch (err) {
    if (!disposed) routerModes.value = { ...routerModes.value, [backend]: previous };
    console.error("[settings] set router mode failed", err);
  } finally {
    if (!disposed) savingRouter.value = false;
  }
}

async function ensureClaudeApiMode() {
  if (disposed) return;
  const defaultMode = defaultRouterModeForBackend("claude");
  if (routerModes.value.claude === defaultMode) return;
  routerModes.value = { ...routerModes.value, claude: defaultMode };
  try {
    await setRouterMode("claude", defaultMode);
  } catch (err) {
    console.error("[settings] set Claude API mode failed", err);
  }
}

async function probe() {
  if (disposed) return;
  await refresh();
  if (disposed) return;
  await Promise.all([checkCodexAppServerUpdate(), loadCodexAccountStatus()]);
}

async function installCodexUpdate() {
  if (disposed) return;
  await installCodexAppServerUpdate();
}

function clearCodexAccountStatus() {
  codexAccountRequestSeq += 1;
  codexAccountStatus.value = null;
  codexAccountLoading.value = false;
}

async function loadCodexAccountStatus() {
  if (!showCodexRuntimeStatus.value) {
    clearCodexAccountStatus();
    return;
  }
  const seq = ++codexAccountRequestSeq;
  codexAccountLoading.value = true;
  try {
    const result = await getCodexAccountQuotaStatus();
    if (!disposed && seq === codexAccountRequestSeq) codexAccountStatus.value = result;
  } catch (err) {
    if (!disposed && seq === codexAccountRequestSeq) {
      codexAccountStatus.value = codexQuotaUnavailableStatus(err);
    }
  } finally {
    if (!disposed && seq === codexAccountRequestSeq) codexAccountLoading.value = false;
  }
}

async function startCodexLogin() {
  if (disposed || codexLoginStarting.value) return;
  codexLoginStarting.value = true;
  try {
    await startCodexAccountLogin();
  } catch (err) {
    if (!disposed) codexAccountStatus.value = codexQuotaUnavailableStatus(err);
  } finally {
    if (!disposed) codexLoginStarting.value = false;
  }
}

async function selectBackend(backend: ChatBackendKind) {
  if (disposed || switchingBackend.value) return;
  switchingBackend.value = backend;
  try {
    await setActiveBackend(backend);
    if (disposed) return;
    await Promise.all([loadProvider(backend), loadRouter(backend), refresh()]);
  } catch (err) {
    console.error("[settings] setActiveBackend failed", err);
  } finally {
    if (!disposed) switchingBackend.value = null;
  }
}

onMounted(async () => {
  disposed = false;
  await Promise.all([loadAllConfig(), refresh()]);
  if (disposed) return;
  await Promise.all([checkCodexAppServerUpdate(), loadCodexAccountStatus()]);
  if (disposed) return;
  await ensureClaudeApiMode();
});

onBeforeUnmount(() => {
  disposed = true;
  codexAccountRequestSeq += 1;
});

watch(showCodexRuntimeStatus, (enabled) => {
  if (enabled) {
    void loadCodexAccountStatus();
  } else {
    clearCodexAccountStatus();
  }
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
      <div class="ui-segmented" role="radiogroup" aria-label="对话后端">
        <button
          v-for="opt in backendOptions"
          :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="selectedBackend === opt.value"
            :data-agent-id="`settings.provider.backend.${opt.value}`"
            :class="{ 'is-active': selectedBackend === opt.value }"
          :disabled="switchingBackend !== null"
          @click="selectBackend(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">接入方式</div>
      <div class="settings-row__control settings-row__control--loose">
        <div
          v-if="selectedBackend === 'codex'"
          class="ui-segmented"
          role="radiogroup"
          aria-label="Codex 接入方式"
        >
          <button
            v-for="opt in codexModeOptions"
            :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="selectedRouterMode === opt.value"
            :data-agent-id="`settings.provider.codex-mode.${opt.value}`"
            :class="{ 'is-active': selectedRouterMode === opt.value }"
            :disabled="savingRouter"
            @click="selectRouterMode(opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
        <span v-else class="settings-row__status-text muted">Claude 使用 API 接入</span>
      </div>
    </div>

    <template v-if="showCodexRuntimeStatus">
      <div class="settings-row settings-row--stacked">
        <div class="settings-row__label">运行时状态</div>
        <div class="settings-row__control settings-row__control--loose">
          <span class="muted" style="display: inline-flex; gap: 4px; align-items: center;">
            <UserRound :size="12" aria-hidden="true" />
            {{ codexRuntimeStatusText }}
          </span>
          <span class="settings-row__status-text muted">当前版本：{{ codexVersionText }}</span>
          <span class="settings-row__status-text muted">{{ codexLoginStatusText }}</span>
          <button
            v-if="codexLoginNeedsAction"
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="settings.provider.codex-login"
            :disabled="codexLoginStarting"
            @click="startCodexLogin"
          >
            <Loader2
              v-if="codexLoginStarting"
              :size="12"
              class="is-spinning"
              aria-hidden="true"
            />
            <LogIn v-else :size="12" aria-hidden="true" />
            {{ codexLoginStarting ? "启动中..." : "登录" }}
          </button>
          <button
            v-if="showCodexUpdateAction || codexAppServerUpdating"
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="settings.provider.codex-update"
            :disabled="codexAppServerUpdating"
            @click="installCodexUpdate"
          >
            <Loader2
              v-if="codexAppServerUpdating || codexAppServerUpdateChecking"
              :size="12"
              class="is-spinning"
              aria-hidden="true"
            />
            <Download v-else :size="12" aria-hidden="true" />
            {{ codexAppServerUpdating ? "更新中..." : codexUpdateLabel }}
          </button>
        <button type="button" class="ui-button ui-button--ghost" data-agent-id="settings.provider.probe" :disabled="probing" @click="probe">
            <RotateCw :size="11" aria-hidden="true" />
            重新检测
          </button>
        </div>
        <div class="settings-row__status muted">
          {{ codexInstallPathText }}
          <span v-if="codexLoginStatusDetail">；{{ codexLoginStatusDetail }}</span>
          <span v-if="codexAppServerUpdateError">；{{ codexAppServerUpdateError }}</span>
        </div>
      </div>
    </template>

    <template v-if="showApiConfig">
      <div class="settings-row settings-row--stacked">
        <div class="settings-row__label">API 来源</div>
        <div class="settings-row__status muted">
          {{ apiDescription }} 默认 URL：{{ apiDefaultUrl }}
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">Base URL</div>
        <input
            type="text"
            class="ui-input"
            :placeholder="apiDefaultUrl"
            data-agent-id="settings.provider.base-url"
            :value="selectedProviderForm.baseUrl ?? ''"
          @input="(e) => (selectedProviderForm.baseUrl = (e.target as HTMLInputElement).value)"
        />
      </div>

      <div class="settings-row">
        <div class="settings-row__label">API key</div>
        <div class="settings-row__control">
          <input
            type="password"
            class="ui-input"
            :placeholder="selectedProviderForm.hasApiKey ? '已保存，留空保留现有值' : apiKeyEnv"
            data-agent-id="settings.provider.api-key"
            :value="selectedProviderForm.apiKey ?? ''"
            @input="(e) => (selectedProviderForm.apiKey = (e.target as HTMLInputElement).value)"
          />
          <button
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="settings.provider.clear-key"
            :disabled="savingProvider || !selectedProviderForm.hasApiKey"
            title="清除已保存的 API key"
            @click="clearProviderKey"
          >
            <Trash2 :size="12" aria-hidden="true" />
            清除
          </button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">密钥状态</div>
        <div class="settings-row__control">
          <span class="muted" style="display: inline-flex; gap: 4px; align-items: center;">
            <KeyRound :size="12" aria-hidden="true" />
            {{ selectedProviderForm.hasApiKey ? "密钥已保存" : "未保存密钥" }}
          </span>
          <button
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="settings.provider.save"
            :disabled="savingProvider"
            @click="saveProvider"
          >
            <Save :size="12" aria-hidden="true" />
            {{ savingProvider ? "保存中..." : "保存" }}
          </button>
        </div>
      </div>
    </template>

    <div
      v-if="selectedDiagnostic"
      class="conn-banner"
      :class="`conn-banner--${selectedDiagnostic.tone}`"
    >
      <Loader2
        v-if="selectedDiagnostic.tone === 'probing'"
        :size="14"
        class="is-spinning"
        aria-hidden="true"
      />
      <AlertTriangle
        v-else
        :size="16"
        aria-hidden="true"
      />
      <div>
        <div class="conn-banner__title">{{ selectedDiagnostic.title }}</div>
        <div class="conn-banner__hint">
          {{ selectedDiagnostic.hint }}
        <button type="button" class="inline-link" data-agent-id="settings.provider.retry-probe" :disabled="probing" @click="probe">
            <RotateCw :size="11" aria-hidden="true" />
            重新检测
          </button>
        </div>
      </div>
    </div>
  </div>
  <RemoteControlSection />
</template>
