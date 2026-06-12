<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  AlertTriangle,
  KeyRound,
  Loader2,
  Network,
  Plug,
  RotateCw,
  Save,
  Trash2,
} from "lucide-vue-next";
import type {
  CCSwitchConfig,
  ChatBackendKind,
  ProviderConfig,
  RouterMode,
} from "@lilia/contracts";
import { useConnectionStatus } from "../../composables/useConnectionStatus";
import {
  getCCSwitchConfig,
  getProviderConfig,
  getRouterMode,
  setCCSwitchConfig,
  setProviderConfig,
  setRouterMode,
} from "../../services/chat";
import ProviderSetupChecklist from "./ProviderSetupChecklist.vue";
import {
  DIRECT_DEFAULT_URLS,
  backendLabel,
  connectionDiagnostic,
  routeLabel,
  runtimeDiagnostic,
} from "./providerDiagnostics";

const {
  report,
  activeBackend,
  setActiveBackend,
  statusFor,
  probing,
  refresh,
  ccSwitch,
} = useConnectionStatus();

const backendOptions: { value: ChatBackendKind; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
];
const routerOptions: { value: RouterMode; label: string }[] = [
  { value: "cc-switch", label: "CC-Switch" },
  { value: "direct", label: "直连" },
];

const switchingBackend = ref<ChatBackendKind | null>(null);
const savingCCSwitch = ref(false);
const savingProvider = ref(false);
const savingRouter = ref(false);
const ccSwitchForm = ref<CCSwitchConfig>({ baseUrl: "http://127.0.0.1:15721" });
const providerForms = ref<Record<ChatBackendKind, ProviderConfig>>({
  claude: { backend: "claude", baseUrl: null, apiKey: null, hasApiKey: false },
  codex: { backend: "codex", baseUrl: null, apiKey: null, hasApiKey: false },
});
const routerModes = ref<Record<ChatBackendKind, RouterMode>>({
  claude: "cc-switch",
  codex: "cc-switch",
});

const selectedBackend = computed(() => activeBackend.value);
const selectedLabel = computed(() => backendLabel(selectedBackend.value));
const selectedStatus = computed(() => statusFor(selectedBackend.value));
const selectedRouterMode = computed(() => routerModes.value[selectedBackend.value]);
const selectedProviderForm = computed(() => providerForms.value[selectedBackend.value]);
const selectedRuntime = computed(() => runtimeDiagnostic(selectedBackend.value, report.value));
const selectedConnection = computed(() =>
  connectionDiagnostic(
    selectedBackend.value,
    selectedStatus.value,
    selectedRouterMode.value,
    ccSwitch.value?.baseUrl ?? ccSwitchForm.value.baseUrl,
  ),
);
const selectedDiagnostic = computed(() => {
  if (probing.value) {
    return { tone: "probing" as const, title: "检查中", hint: "正在读取本机运行时和连接配置。" };
  }
  const runtime = selectedRuntime.value;
  if (runtime && runtime.tone === "err") return runtime;
  return selectedConnection.value ?? runtime;
});
const directDefaultUrl = computed(() => DIRECT_DEFAULT_URLS[selectedBackend.value]);
const directApiKeyEnv = computed(() =>
  selectedBackend.value === "codex" ? "OPENAI_API_KEY" : "ANTHROPIC_API_KEY",
);
const directDescription = computed(() =>
  selectedBackend.value === "codex"
    ? "Base URL 留空时使用 OpenAI API；也可填写 OpenAI 兼容端点。"
    : "Base URL 留空时使用 Anthropic API；也可填写 Anthropic 兼容端点。",
);

async function loadCCSwitch() {
  try {
    ccSwitchForm.value = await getCCSwitchConfig();
  } catch (err) {
    console.error("[settings] load cc-switch config failed", err);
  }
}

async function loadProvider(backend: ChatBackendKind) {
  try {
    const config = await getProviderConfig(backend);
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
    routerModes.value = {
      ...routerModes.value,
      [backend]: mode === "direct" ? "direct" : "cc-switch",
    };
  } catch (err) {
    console.error("[settings] load router mode failed", err);
  }
}

async function loadAllConfig() {
  await Promise.all([
    loadCCSwitch(),
    loadProvider("claude"),
    loadProvider("codex"),
    loadRouter("claude"),
    loadRouter("codex"),
  ]);
}

async function saveCCSwitch() {
  const cfg: CCSwitchConfig = { baseUrl: ccSwitchForm.value.baseUrl?.trim() || null };
  savingCCSwitch.value = true;
  try {
    await setCCSwitchConfig(cfg);
    await refresh();
  } catch (err) {
    console.error("[settings] setCCSwitchConfig failed", err);
  } finally {
    savingCCSwitch.value = false;
  }
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
  const backend = selectedBackend.value;
  savingProvider.value = true;
  try {
    await setProviderConfig(normalizedProviderConfig(false));
    await loadProvider(backend);
    await refresh();
  } catch (err) {
    console.error("[settings] save provider config failed", err);
  } finally {
    savingProvider.value = false;
  }
}

async function clearProviderKey() {
  const backend = selectedBackend.value;
  savingProvider.value = true;
  try {
    await setProviderConfig({ ...normalizedProviderConfig(true), apiKey: null, clearApiKey: true });
    providerForms.value[backend].apiKey = null;
    await loadProvider(backend);
    await refresh();
  } catch (err) {
    console.error("[settings] clear provider key failed", err);
  } finally {
    savingProvider.value = false;
  }
}

async function selectRouterMode(mode: RouterMode) {
  const backend = selectedBackend.value;
  if (savingRouter.value || routerModes.value[backend] === mode) return;
  const previous = routerModes.value[backend];
  routerModes.value = { ...routerModes.value, [backend]: mode };
  savingRouter.value = true;
  try {
    await setRouterMode(backend, mode);
    await refresh();
  } catch (err) {
    routerModes.value = { ...routerModes.value, [backend]: previous };
    console.error("[settings] set router mode failed", err);
  } finally {
    savingRouter.value = false;
  }
}

async function probe() {
  await refresh();
}

async function selectBackend(backend: ChatBackendKind) {
  if (switchingBackend.value) return;
  switchingBackend.value = backend;
  try {
    await setActiveBackend(backend);
    await Promise.all([loadProvider(backend), loadRouter(backend), refresh()]);
  } catch (err) {
    console.error("[settings] setActiveBackend failed", err);
  } finally {
    switchingBackend.value = null;
  }
}

onMounted(async () => {
  await Promise.all([loadAllConfig(), refresh()]);
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

    <ProviderSetupChecklist
      :backend="selectedBackend"
      :report="report"
      :status="selectedStatus"
      :router-mode="selectedRouterMode"
      :cc-switch-base-url="ccSwitch?.baseUrl ?? ccSwitchForm.baseUrl"
      :probing="probing"
    />

    <div class="settings-row">
      <div class="settings-row__label">使用</div>
      <div class="ui-segmented" role="radiogroup" aria-label="对话后端">
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
      <div class="settings-row__label">连接模式</div>
      <div class="settings-row__control settings-row__control--loose">
        <div class="ui-segmented" role="radiogroup" :aria-label="`${selectedLabel} 连接模式`">
          <button
            v-for="opt in routerOptions"
            :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="selectedRouterMode === opt.value"
            :class="{ 'is-active': selectedRouterMode === opt.value }"
            :disabled="savingRouter"
            @click="selectRouterMode(opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
        <span class="settings-row__status-text muted">
          {{ selectedLabel }} 当前使用 {{ routeLabel(selectedRouterMode) }}
        </span>
      </div>
    </div>

    <template v-if="selectedRouterMode === 'cc-switch'">
      <div class="settings-row">
        <div class="settings-row__label">代理 URL</div>
        <div class="settings-row__control">
          <input
            type="text"
            class="ui-input"
            placeholder="http://127.0.0.1:15721"
            :value="ccSwitchForm.baseUrl ?? ''"
            @input="(e) => (ccSwitchForm.baseUrl = (e.target as HTMLInputElement).value)"
          />
          <button type="button" class="ui-button ui-button--ghost" :disabled="savingCCSwitch" @click="saveCCSwitch">
            <Save :size="12" aria-hidden="true" />
            {{ savingCCSwitch ? "保存中..." : "保存" }}
          </button>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="settings-row settings-row--stacked">
        <div class="settings-row__label">直连说明</div>
        <div class="settings-row__status muted">
          {{ directDescription }} 默认 URL：{{ directDefaultUrl }}
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">Base URL</div>
        <input
          type="text"
          class="ui-input"
          :placeholder="directDefaultUrl"
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
            :placeholder="selectedProviderForm.hasApiKey ? '已保存，留空保留现有值' : directApiKeyEnv"
            :value="selectedProviderForm.apiKey ?? ''"
            @input="(e) => (selectedProviderForm.apiKey = (e.target as HTMLInputElement).value)"
          />
          <button
            type="button"
            class="ui-button ui-button--ghost"
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
      <component
        :is="selectedDiagnostic.tone === 'ok' ? Plug : AlertTriangle"
        v-else
        :size="16"
        aria-hidden="true"
      />
      <div>
        <div class="conn-banner__title">{{ selectedDiagnostic.title }}</div>
        <div class="conn-banner__hint">
          {{ selectedDiagnostic.hint }}
          <button type="button" class="inline-link" :disabled="probing" @click="probe">
            <RotateCw :size="11" aria-hidden="true" />
            重新检测
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
