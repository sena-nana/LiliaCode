<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { AlertTriangle, KeyRound, MessageSquarePlus, Plug, Save, Sparkles, Trash2 } from "lucide-vue-next";
import {
  DEFAULT_SUGGESTION_SOURCE,
  SUGGESTION_SOURCE_SETTING_ORDER,
  suggestionSourceSettingLabel,
  type AssistantAIConfig,
  type AssistantAITestResult,
  type SuggestionSettings,
  type SuggestionSource,
} from "@lilia/contracts";
import {
  getConversationSuggestionSettings,
  getAssistantAIConfig,
  setConversationSuggestionSettings,
  setAssistantAIConfig,
  testAssistantAIConnection,
} from "../../services/chat";

const assistantAIForm = ref<AssistantAIConfig>({
  baseUrl: null,
  apiKey: null,
  model: null,
  codexAccountSparkEnabled: false,
  hasApiKey: false,
});
const suggestionSettings = ref<SuggestionSettings>({
  enabled: true,
  source: DEFAULT_SUGGESTION_SOURCE,
});
const savingAssistantAI = ref(false);
const savingSuggestions = ref(false);
const testingAssistantAI = ref(false);
const assistantAIResult = ref<AssistantAITestResult | null>(null);
let disposed = false;

const suggestionSourceOptions: Array<{ value: SuggestionSource; label: string }> = [
  ...SUGGESTION_SOURCE_SETTING_ORDER.map((value) => ({
    value,
    label: suggestionSourceSettingLabel(value),
  })),
];

const assistantAIBannerHint = computed(() => {
  const r = assistantAIResult.value;
  if (!r) return "";
  if (!r.ok) return r.error ?? "未知错误";
  if (r.modelMatched === false) {
    return `已连接，但配置的模型不在 /models 列表里（共 ${r.models?.length ?? 0} 个可用）。请确认模型名拼写。`;
  }
  if (r.modelMatched === true) {
    return "已连接，配置的模型在端点 /models 列表里。";
  }
  return "已连接（端点未返回 /models 列表，无法确认模型名是否有效）。";
});

function normalizedAssistantAI(): AssistantAIConfig {
  return {
    baseUrl: assistantAIForm.value.baseUrl?.trim() || null,
    apiKey: assistantAIForm.value.apiKey?.trim() || null,
    model: assistantAIForm.value.model?.trim() || null,
    codexAccountSparkEnabled: assistantAIForm.value.codexAccountSparkEnabled === true,
    hasApiKey: assistantAIForm.value.hasApiKey,
  };
}

async function loadAssistantAI() {
  try {
    const config = await getAssistantAIConfig();
    if (disposed) return;
    assistantAIForm.value = { ...config, apiKey: null };
  }
  catch (err) { console.error("[settings] load assistant ai config failed", err); }
}

async function loadSuggestionSettings() {
  try {
    const settings = await getConversationSuggestionSettings();
    if (!disposed) suggestionSettings.value = settings;
  }
  catch (err) { console.error("[settings] load suggestion settings failed", err); }
}

async function saveAssistantAI() {
  if (disposed) return;
  savingAssistantAI.value = true;
  try {
    await setAssistantAIConfig(normalizedAssistantAI());
    if (disposed) return;
    await loadAssistantAI();
  } catch (err) { console.error("[settings] saveAssistantAI failed", err); }
  finally { if (!disposed) savingAssistantAI.value = false; }
}

async function clearAssistantAIKey() {
  if (disposed) return;
  savingAssistantAI.value = true;
  try {
    await setAssistantAIConfig({ ...normalizedAssistantAI(), apiKey: null, clearApiKey: true });
    if (disposed) return;
    assistantAIForm.value.apiKey = null;
    await loadAssistantAI();
  } catch (err) { console.error("[settings] clearAssistantAIKey failed", err); }
  finally { if (!disposed) savingAssistantAI.value = false; }
}

async function testAssistantAI() {
  if (disposed) return;
  testingAssistantAI.value = true;
  assistantAIResult.value = null;
  try {
    const result = await testAssistantAIConnection(normalizedAssistantAI());
    if (!disposed) assistantAIResult.value = result;
  } catch (err) {
    if (!disposed) assistantAIResult.value = {
      ok: false, error: String(err), models: null, modelMatched: null,
    };
  } finally { if (!disposed) testingAssistantAI.value = false; }
}

async function setSuggestionPatch(patch: Partial<SuggestionSettings>) {
  if (disposed) return;
  const next: SuggestionSettings = { ...suggestionSettings.value, ...patch };
  suggestionSettings.value = next;
  savingSuggestions.value = true;
  try {
    await setConversationSuggestionSettings(next);
  } catch (err) {
    console.error("[settings] save suggestion settings failed", err);
  } finally {
    if (!disposed) savingSuggestions.value = false;
  }
}

onMounted(() => {
  disposed = false;
  void Promise.all([loadAssistantAI(), loadSuggestionSettings()]);
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
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
      <div class="settings-row__label">基础 URL</div>
      <input
        type="text"
        class="ui-input"
        placeholder="https://api.example.com/v1"
        :value="assistantAIForm.baseUrl ?? ''"
        @input="(e) => (assistantAIForm.baseUrl = (e.target as HTMLInputElement).value)"
      />
    </div>
    <div class="settings-row">
      <div class="settings-row__label">API 密钥</div>
      <div style="display: flex; gap: 8px; align-items: center;">
        <input
          type="password"
          class="ui-input"
          :placeholder="assistantAIForm.hasApiKey ? '已保存，留空保留现有值' : 'sk-...'"
          :value="assistantAIForm.apiKey ?? ''"
          @input="(e) => (assistantAIForm.apiKey = (e.target as HTMLInputElement).value)"
        />
        <button
          type="button"
          class="ui-button ui-button--ghost"
          :disabled="savingAssistantAI || !assistantAIForm.hasApiKey"
          title="清除已保存的 API 密钥"
          @click="clearAssistantAIKey"
        >
          <Trash2 :size="12" aria-hidden="true" />
          清除
        </button>
      </div>
    </div>
    <div class="settings-row">
      <div class="settings-row__label">模型</div>
      <input
        type="text"
        class="ui-input"
        placeholder="gpt-4o-mini"
        :value="assistantAIForm.model ?? ''"
        @input="(e) => (assistantAIForm.model = (e.target as HTMLInputElement).value)"
      />
    </div>
    <div class="settings-row">
      <div class="settings-row__label">Codex 官方账号</div>
      <div class="ui-segmented" role="radiogroup" aria-label="Codex 官方账号 Spark 辅助模型">
        <button
          type="button"
          role="radio"
          :aria-checked="assistantAIForm.codexAccountSparkEnabled"
          :class="{ 'is-active': assistantAIForm.codexAccountSparkEnabled }"
          @click="assistantAIForm.codexAccountSparkEnabled = true"
        >
          使用 Spark
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="!assistantAIForm.codexAccountSparkEnabled"
          :class="{ 'is-active': !assistantAIForm.codexAccountSparkEnabled }"
          @click="assistantAIForm.codexAccountSparkEnabled = false"
        >
          关闭
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">连通性</div>
      <div style="display: flex; gap: 8px; align-items: center;">
        <span class="muted" style="display: inline-flex; gap: 4px; align-items: center;">
          <KeyRound :size="12" aria-hidden="true" />
          {{ assistantAIForm.hasApiKey ? "密钥已保存" : "未保存密钥" }}
        </span>
        <button
          type="button"
          class="ui-button ui-button--ghost"
          :disabled="savingAssistantAI || testingAssistantAI"
          @click="saveAssistantAI"
        >
          <Save :size="12" aria-hidden="true" />
          {{ savingAssistantAI ? "保存中…" : "保存" }}
        </button>
        <button
          type="button"
          class="ui-button ui-button--ghost"
          :disabled="testingAssistantAI || savingAssistantAI"
          title="GET {baseUrl}/models，不消耗 token"
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
        <MessageSquarePlus :size="14" aria-hidden="true" />
        新对话建议
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">启用状态</div>
      <div class="ui-segmented" role="radiogroup" aria-label="新对话建议">
        <button
          type="button"
          role="radio"
          :aria-checked="suggestionSettings.enabled"
          :class="{ 'is-active': suggestionSettings.enabled }"
          :disabled="savingSuggestions"
          @click="setSuggestionPatch({ enabled: true })"
        >
          开启
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="!suggestionSettings.enabled"
          :class="{ 'is-active': !suggestionSettings.enabled }"
          :disabled="savingSuggestions"
          @click="setSuggestionPatch({ enabled: false })"
        >
          关闭
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">生成来源</div>
      <div class="ui-segmented" role="radiogroup" aria-label="建议生成来源">
        <button
          v-for="opt in suggestionSourceOptions"
          :key="opt.value"
          type="button"
          role="radio"
          :aria-checked="suggestionSettings.source === opt.value"
          :class="{ 'is-active': suggestionSettings.source === opt.value }"
          :disabled="savingSuggestions"
          @click="setSuggestionPatch({ source: opt.value })"
        >
          {{ opt.label }}
        </button>
      </div>
    </div>
  </div>
</template>
