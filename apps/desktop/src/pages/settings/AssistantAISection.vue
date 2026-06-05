<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { AlertTriangle, Plug, Save, Sparkles } from "lucide-vue-next";
import type {
  AssistantAIConfig,
  AssistantAITestResult,
  SuggestionSettings,
  SuggestionSource,
} from "@lilia/contracts";
import {
  getConversationSuggestionSettings,
  getAssistantAIConfig,
  setConversationSuggestionSettings,
  setAssistantAIConfig,
  testAssistantAIConnection,
} from "../../services/chat";

const assistantAIForm = ref<AssistantAIConfig>({ baseUrl: null, apiKey: null, model: null });
const suggestionSettings = ref<SuggestionSettings>({ enabled: true, source: "assistant-ai" });
const savingAssistantAI = ref(false);
const savingSuggestions = ref(false);
const testingAssistantAI = ref(false);
const assistantAIResult = ref<AssistantAITestResult | null>(null);

const suggestionSourceOptions: Array<{ value: SuggestionSource; label: string }> = [
  { value: "assistant-ai", label: "辅助模型" },
  { value: "provider", label: "当前 Provider" },
];

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

async function loadSuggestionSettings() {
  try { suggestionSettings.value = await getConversationSuggestionSettings(); }
  catch (err) { console.error("[settings] load suggestion settings failed", err); }
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

async function setSuggestionPatch(patch: Partial<SuggestionSettings>) {
  const next: SuggestionSettings = { ...suggestionSettings.value, ...patch };
  suggestionSettings.value = next;
  savingSuggestions.value = true;
  try {
    await setConversationSuggestionSettings(next);
  } catch (err) {
    console.error("[settings] save suggestion settings failed", err);
  } finally {
    savingSuggestions.value = false;
  }
}

onMounted(() => {
  void Promise.all([loadAssistantAI(), loadSuggestionSettings()]);
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

    <div class="settings-row">
      <div class="settings-row__label">
        <div>新对话建议</div>
        <div class="settings-row__hint">为空白草稿生成两条低成本继续方向。</div>
      </div>
      <div class="segmented" role="radiogroup" aria-label="新对话建议">
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
      <div class="settings-row__label">
        <div>生成来源</div>
        <div class="settings-row__hint">辅助模型走 OpenAI 兼容接口；当前 Provider 复用全局连接。</div>
      </div>
      <div class="segmented" role="radiogroup" aria-label="建议生成来源">
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
</template>
