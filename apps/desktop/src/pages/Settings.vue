<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  Moon, Sun, Loader2,
  Plug, AlertTriangle, Save, Network, FolderOpen, FolderTree, Sparkles,
} from "lucide-vue-next";
import { useTheme } from "../composables/useTheme";
import { useConnectionStatus } from "../composables/useConnectionStatus";
import {
  getAssistantAIConfig,
  getCCSwitchConfig,
  setAssistantAIConfig,
  setCCSwitchConfig,
  setRouterMode,
  testAssistantAIConnection,
} from "../services/chat";
import {
  getProjectSettings,
  pickFolder,
  setProjectSettings,
} from "../services/projects";
import type {
  AgentInteractionSettings,
  AssistantAIConfig,
  AssistantAITestResult,
  CCSwitchConfig,
  ChatBackendKind,
  ProjectSettings,
} from "@lilia/contracts";
import { useAgentInteractionSettings } from "../composables/useAgentInteractionSettings";

const { theme, setTheme } = useTheme();
const {
  activeBackend,
  setActiveBackend,
  statusFor,
  probing,
  refresh,
  nodeAvailable,
  codexCliAvailable,
  ccSwitch,
} = useConnectionStatus();

const backendOptions: { value: ChatBackendKind; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
];
const switchingBackend = ref<ChatBackendKind | null>(null);

const selectedBackend = computed(() => activeBackend.value);
const selectedStatus = computed(() => statusFor(selectedBackend.value));
const isClaude = computed(() => selectedBackend.value === "claude");

const selectedRuntimeIssue = computed<string | null>(() => {
  if (probing.value) return null;
  if (!nodeAvailable.value) return "未找到 node（v18+），SDK 需要本机 Node 运行时。";
  if (!isClaude.value && !codexCliAvailable.value) return "未找到 codex CLI。请先 npm i -g @openai/codex 再重新检测。";
  return null;
});

const selectedOk = computed(() => selectedStatus.value?.connectionMode === "cc-switch");

const selectedHint = computed(() => {
  const s = selectedStatus.value;
  if (!s) return "正在检测…";
  if (s.connectionMode === "cc-switch") return `经 ${s.effectiveUrl ?? "—"} 转发到 CC-Switch 选中的上游 provider。`;
  return `代理 ${ccSwitch.value?.baseUrl ?? "—"} 不可达。请检查 CC-Switch 是否在运行，或修改下方 URL。`;
});

const ccSwitchForm = ref<CCSwitchConfig>({ baseUrl: "http://127.0.0.1:15721" });
const savingCCSwitch = ref(false);

async function loadConfig() {
  try { ccSwitchForm.value = await getCCSwitchConfig(); }
  catch (err) { console.error("[settings] load cc-switch config failed", err); }
}

// 历史 direct 模式 UI 已收回，挂载时把两个 backend 的 router 强制对齐到 cc-switch。
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

// ---- 辅助模型（Assistant AI）----
// 独立 OpenAI 兼容配置：不参与 Agent，仅供 Memory 助手等周边模块消费。
const assistantAIForm = ref<AssistantAIConfig>({ baseUrl: null, apiKey: null, model: null });
const savingAssistantAI = ref(false);
const testingAssistantAI = ref(false);
const assistantAIResult = ref<AssistantAITestResult | null>(null);

const assistantAIBannerHint = computed(() => {
  const r = assistantAIResult.value;
  if (!r) return "";
  if (!r.ok) return r.error ?? "未知错误";
  if (r.modelMatched === false) {
    return `已连接，但配置的 model 不在 /models 列表里（共 ${r.models?.length ?? 0} 个可用）。请确认模型名拼写。`;
  }
  if (r.modelMatched === true) {
    return "已连接，配置的 model 在端点 /models 列表里。";
  }
  return "已连接（端点未返回 /models 列表，无法确认 model 名是否有效）。";
});

function normalizedAssistantAI(): AssistantAIConfig {
  return {
    baseUrl: assistantAIForm.value.baseUrl?.trim() || null,
    apiKey: assistantAIForm.value.apiKey?.trim() || null,
    model: assistantAIForm.value.model?.trim() || null,
  };
}

async function loadAssistantAI() {
  try { assistantAIForm.value = await getAssistantAIConfig(); }
  catch (err) { console.error("[settings] load assistant ai config failed", err); }
}

async function saveAssistantAI() {
  savingAssistantAI.value = true;
  try {
    await setAssistantAIConfig(normalizedAssistantAI());
  } catch (err) { console.error("[settings] saveAssistantAI failed", err); }
  finally { savingAssistantAI.value = false; }
}

async function testAssistantAI() {
  testingAssistantAI.value = true;
  assistantAIResult.value = null;
  try {
    assistantAIResult.value = await testAssistantAIConnection(normalizedAssistantAI());
  } catch (err) {
    assistantAIResult.value = {
      ok: false, error: String(err), models: null, modelMatched: null,
    };
  } finally { testingAssistantAI.value = false; }
}

// ---- Agent 交互 ----
const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);

async function loadAgentInteraction() {
  try {
    await agentInteractionStore.load();
  } catch (err) {
    agentInteractionError.value = `读取 Agent 交互设置失败：${String(err)}`;
  }
}

async function setNonInterruptMode(nonInterruptMode: boolean) {
  await setAgentInteraction({ nonInterruptMode });
}

async function setDebugMode(debug: boolean) {
  await setAgentInteraction({ debug });
}

async function setAgentInteraction(patch: Partial<AgentInteractionSettings>) {
  const next = { ...agentInteraction.value, ...patch };
  if (
    next.nonInterruptMode === agentInteraction.value.nonInterruptMode &&
    next.debug === agentInteraction.value.debug
  ) {
    return;
  }
  savingAgentInteraction.value = true;
  agentInteractionError.value = null;
  try {
    await agentInteractionStore.update(patch);
  } catch (err) {
    agentInteractionError.value = `保存 Agent 交互设置失败：${String(err)}`;
  } finally {
    savingAgentInteraction.value = false;
  }
}

// ---- 项目偏好 ----
const projectSettings = ref<ProjectSettings>({ cloneParentDir: null });
const savingProject = ref(false);
const projectError = ref<string | null>(null);

async function loadProjectSettings() {
  try {
    projectSettings.value = await getProjectSettings();
  } catch (err) {
    projectError.value = `读取项目偏好失败：${String(err)}`;
  }
}

async function pickCloneParent() {
  projectError.value = null;
  try {
    const picked = await pickFolder({
      title: "选择默认 clone 父目录",
      defaultPath: projectSettings.value.cloneParentDir,
    });
    if (!picked) return;
    projectSettings.value = { ...projectSettings.value, cloneParentDir: picked };
    await persistProjectSettings();
  } catch (err) {
    projectError.value = `选择文件夹失败：${String(err)}`;
  }
}

async function persistProjectSettings() {
  savingProject.value = true;
  try {
    await setProjectSettings(projectSettings.value);
  } catch (err) {
    projectError.value = `保存项目偏好失败：${String(err)}`;
  } finally {
    savingProject.value = false;
  }
}

onMounted(async () => {
  await lockRouters();
  await Promise.all([
    loadConfig(),
    loadAssistantAI(),
    loadAgentInteraction(),
    loadProjectSettings(),
    refresh(),
  ]);
});
</script>

<template>
  <section>
    <div class="page-header">
      <div>
        <h1>设置</h1>
        <p>对话统一经 CC-Switch 本地代理转发。</p>
      </div>
    </div>

    <div class="card">
      <h2>外观</h2>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>主题</div>
          <div class="settings-row__hint">立即生效并记忆到本地。</div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="主题">
          <button
            type="button"
            role="radio"
            :aria-checked="theme === 'dark'"
            :class="{ 'is-active': theme === 'dark' }"
            @click="setTheme('dark')"
          >
            <Moon :size="14" aria-hidden="true" />
            暗色
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="theme === 'light'"
            :class="{ 'is-active': theme === 'light' }"
            @click="setTheme('light')"
          >
            <Sun :size="14" aria-hidden="true" />
            浅色
          </button>
        </div>
      </div>
      <div class="settings-row">
        <div class="settings-row__label">
          <div>语言</div>
          <div class="settings-row__hint">界面语言</div>
        </div>
        <span class="muted">简体中文</span>
      </div>
    </div>

    <div class="card">
      <h2>
        <span class="card-h2__title">
          <Network :size="14" aria-hidden="true" />
          连接
        </span>
      </h2>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>使用</div>
          <div class="settings-row__hint">切换全局 Agent provider。</div>
        </div>
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
        <div class="settings-row__label">
          <div>代理 URL</div>
          <div class="settings-row__hint">默认 <code>http://127.0.0.1:15721</code>。</div>
        </div>
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
          <div class="conn-banner__title">{{ selectedOk ? "已连接" : "未连接" }}</div>
          <div class="conn-banner__hint">{{ selectedHint }}</div>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>
        <span class="card-h2__title">
          <Sparkles :size="14" aria-hidden="true" />
          辅助模型
        </span>
      </h2>
      <p class="muted" style="margin: 0 0 8px;">
        OpenAI 兼容的低成本模型，承接 Memory 助手、摘要等周边计算，不参与对话本身。
      </p>

      <div class="settings-row">
        <div class="settings-row__label"><div>Base URL</div></div>
        <input
          type="text"
          class="text-input"
          placeholder="https://api.example.com/v1"
          :value="assistantAIForm.baseUrl ?? ''"
          @input="(e) => (assistantAIForm.baseUrl = (e.target as HTMLInputElement).value)"
        />
      </div>
      <div class="settings-row">
        <div class="settings-row__label"><div>API Key</div></div>
        <input
          type="password"
          class="text-input"
          placeholder="sk-..."
          :value="assistantAIForm.apiKey ?? ''"
          @input="(e) => (assistantAIForm.apiKey = (e.target as HTMLInputElement).value)"
        />
      </div>
      <div class="settings-row">
        <div class="settings-row__label"><div>Model</div></div>
        <input
          type="text"
          class="text-input"
          placeholder="gpt-4o-mini"
          :value="assistantAIForm.model ?? ''"
          @input="(e) => (assistantAIForm.model = (e.target as HTMLInputElement).value)"
        />
      </div>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>连通性</div>
          <div class="settings-row__hint">GET <code>{baseUrl}/models</code>，不消耗 token。</div>
        </div>
        <div style="display: flex; gap: 8px; align-items: center;">
          <button
            type="button"
            class="ghost"
            :disabled="savingAssistantAI || testingAssistantAI"
            @click="saveAssistantAI"
          >
            <Save :size="12" aria-hidden="true" />
            {{ savingAssistantAI ? "保存中…" : "保存" }}
          </button>
          <button
            type="button"
            class="ghost"
            :disabled="testingAssistantAI || savingAssistantAI"
            @click="testAssistantAI"
          >
            <Plug :size="12" aria-hidden="true" />
            {{ testingAssistantAI ? "测试中…" : "测试连接" }}
          </button>
        </div>
      </div>

      <div
        v-if="assistantAIResult"
        class="conn-banner"
        :class="assistantAIResult.ok ? 'conn-banner--ok' : 'conn-banner--err'"
      >
        <component :is="assistantAIResult.ok ? Plug : AlertTriangle" :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">{{ assistantAIResult.ok ? "可达" : "不可达" }}</div>
          <div class="conn-banner__hint">{{ assistantAIBannerHint }}</div>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>
        <span class="card-h2__title">
          <Sparkles :size="14" aria-hidden="true" />
          Agent 交互
        </span>
      </h2>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>非打断模式</div>
          <div class="settings-row__hint">权限、提问和计划确认留在时间线卡片中处理。</div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="非打断模式">
          <button
            type="button"
            role="radio"
            :aria-checked="!agentInteraction.nonInterruptMode"
            :class="{ 'is-active': !agentInteraction.nonInterruptMode }"
            :disabled="savingAgentInteraction"
            @click="setNonInterruptMode(false)"
          >
            关闭
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="agentInteraction.nonInterruptMode"
            :class="{ 'is-active': agentInteraction.nonInterruptMode }"
            :disabled="savingAgentInteraction"
            @click="setNonInterruptMode(true)"
          >
            开启
          </button>
        </div>
      </div>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>Debug 面板</div>
          <div class="settings-row__hint">在对话侧栏加入临时事件注入面板。</div>
        </div>
        <div class="segmented" role="radiogroup" aria-label="Debug 面板">
          <button
            type="button"
            role="radio"
            :aria-checked="!agentInteraction.debug"
            :class="{ 'is-active': !agentInteraction.debug }"
            :disabled="savingAgentInteraction"
            @click="setDebugMode(false)"
          >
            关闭
          </button>
          <button
            type="button"
            role="radio"
            :aria-checked="agentInteraction.debug"
            :class="{ 'is-active': agentInteraction.debug }"
            :disabled="savingAgentInteraction"
            @click="setDebugMode(true)"
          >
            开启
          </button>
        </div>
      </div>

      <div v-if="agentInteractionError" class="conn-banner conn-banner--err">
        <AlertTriangle :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">Agent 交互</div>
          <div class="conn-banner__hint">{{ agentInteractionError }}</div>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>
        <span class="card-h2__title">
          <FolderTree :size="14" aria-hidden="true" />
          项目
        </span>
      </h2>

      <div class="settings-row">
        <div class="settings-row__label">
          <div>Clone 默认父目录</div>
        </div>
        <div style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap; justify-content: flex-end;">
          <span class="muted" style="max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
            {{ projectSettings.cloneParentDir || "未设置（用家目录）" }}
          </span>
          <button type="button" class="ghost" :disabled="savingProject" @click="pickCloneParent">
            <FolderOpen :size="12" aria-hidden="true" />
            选择
          </button>
        </div>
      </div>

      <div v-if="projectError" class="conn-banner conn-banner--err">
        <AlertTriangle :size="16" aria-hidden="true" />
        <div>
          <div class="conn-banner__title">项目偏好</div>
          <div class="conn-banner__hint">{{ projectError }}</div>
        </div>
      </div>
    </div>

    <div class="card">
      <h2>关于</h2>
      <ul class="kv">
        <li><span>名称</span><span>LiliaCode</span></li>
        <li><span>版本</span><span>0.1.0</span></li>
      </ul>
    </div>
  </section>
</template>
