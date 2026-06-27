<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { AlertTriangle, KeyRound, Plug, Save, Sparkles } from "lucide-vue-next";
import type {
  AssistantAIConfig,
  AssistantAIModelPoolItem,
  AssistantAIModelsResult,
  AssistantAITestResult,
} from "@lilia/contracts";
import {
  fetchAssistantAIModels,
  getAssistantAIConfig,
  setAssistantAIConfig,
  testAssistantAIConnection,
} from "../../services/chat";

const assistantAIForm = ref<AssistantAIConfig>({
  baseUrl: null,
  apiKey: null,
  model: null,
  modelPool: [],
  codexAccountSparkEnabled: false,
  hasApiKey: false,
});
const savingAssistantAI = ref(false);
const fetchingModels = ref(false);
const testingAssistantAI = ref(false);
const assistantAIResult = ref<AssistantAITestResult | null>(null);
const fetchModelsResult = ref<AssistantAIModelsResult | null>(null);
let disposed = false;

const assistantAIBannerHint = computed(() => {
  const r = assistantAIResult.value;
  if (!r) return "";
  if (!r.ok) return r.error ?? "未知错误";
  if (r.modelMatched === false) {
    return `已连接，但旧模型不在 /models 列表里（共 ${r.models?.length ?? 0} 个可用）。`;
  }
  if (r.modelMatched === true) {
    return "已连接，旧模型在端点 /models 列表里。";
  }
  return "已连接（端点未返回 /models 列表）。";
});

function modelPool(): AssistantAIModelPoolItem[] {
  return assistantAIForm.value.modelPool ?? [];
}

function normalizedAssistantAI(): AssistantAIConfig {
  return {
    baseUrl: assistantAIForm.value.baseUrl?.trim() || null,
    apiKey: assistantAIForm.value.apiKey?.trim() || null,
    model: assistantAIForm.value.model?.trim() || null,
    modelPool: modelPool(),
    codexAccountSparkEnabled: assistantAIForm.value.codexAccountSparkEnabled === true,
    hasApiKey: assistantAIForm.value.hasApiKey,
  };
}

function mergeFetchedModels(models: AssistantAIModelPoolItem[]) {
  const byId = new Map(modelPool().map((item) => [item.id, item]));
  for (const item of models) {
    const id = item.id.trim();
    if (!id) continue;
    const existing = byId.get(id);
    byId.set(id, {
      id,
      label: existing?.label?.trim() || item.label.trim() || id,
      source: "remote",
      backend: "codex",
    });
  }
  assistantAIForm.value.modelPool = [...byId.values()];
}

async function loadAssistantAI() {
  try {
    const config = await getAssistantAIConfig();
    if (disposed) return;
    assistantAIForm.value = {
      ...config,
      apiKey: null,
      modelPool: config.modelPool ?? [],
    };
  }
  catch (err) { console.error("[settings] load assistant ai config failed", err); }
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

async function fetchModels() {
  if (disposed) return;
  fetchingModels.value = true;
  fetchModelsResult.value = null;
  try {
    const result = await fetchAssistantAIModels(normalizedAssistantAI());
    if (disposed) return;
    fetchModelsResult.value = result;
    if (result.ok) mergeFetchedModels(result.models);
  } catch (err) {
    if (!disposed) fetchModelsResult.value = { ok: false, error: String(err), models: [] };
  } finally { if (!disposed) fetchingModels.value = false; }
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

function updateModelLabel(id: string, label: string) {
  assistantAIForm.value.modelPool = modelPool().map((item) =>
    item.id === id ? { ...item, label } : item
  );
}

onMounted(() => {
  disposed = false;
  void loadAssistantAI();
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
        模型池配置
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">基础 URL</div>
      <input
        type="text"
        class="ui-input"
        placeholder="https://api.example.com/v1"
        data-agent-id="settings.assistant-ai.base-url"
        :value="assistantAIForm.baseUrl ?? ''"
        @input="(e) => (assistantAIForm.baseUrl = (e.target as HTMLInputElement).value)"
      />
    </div>
    <div class="settings-row">
      <div class="settings-row__label">API 密钥</div>
      <input
        type="password"
        class="ui-input"
        :placeholder="assistantAIForm.hasApiKey ? '已保存，留空保留现有值' : 'sk-...'"
        data-agent-id="settings.assistant-ai.api-key"
        :value="assistantAIForm.apiKey ?? ''"
        @input="(e) => (assistantAIForm.apiKey = (e.target as HTMLInputElement).value)"
      />
    </div>
    <div class="settings-row">
      <div class="settings-row__label">模型</div>
      <button
        type="button"
        class="ui-button ui-button--ghost"
        data-agent-id="settings.assistant-ai.fetch-models"
        :disabled="fetchingModels || savingAssistantAI"
        title="GET {baseUrl}/models，不消耗 token"
        @click="fetchModels"
      >
        <Plug :size="12" aria-hidden="true" />
        {{ fetchingModels ? "获取中…" : "获取模型" }}
      </button>
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
          data-agent-id="settings.assistant-ai.save"
          :disabled="savingAssistantAI || testingAssistantAI || fetchingModels"
          @click="saveAssistantAI"
        >
          <Save :size="12" aria-hidden="true" />
          {{ savingAssistantAI ? "保存中…" : "保存" }}
        </button>
        <button
          type="button"
          class="ui-button ui-button--ghost"
          data-agent-id="settings.assistant-ai.test"
          :disabled="testingAssistantAI || savingAssistantAI || fetchingModels"
          title="GET {baseUrl}/models，不消耗 token"
          @click="testAssistantAI"
        >
          <Plug :size="12" aria-hidden="true" />
          {{ testingAssistantAI ? "测试中…" : "测试连接" }}
        </button>
      </div>
    </div>

    <div
      v-if="fetchModelsResult"
      class="conn-banner"
      :class="fetchModelsResult.ok ? 'conn-banner--ok' : 'conn-banner--err'"
    >
      <component :is="fetchModelsResult.ok ? Plug : AlertTriangle" :size="16" aria-hidden="true" />
      <div>
        <div class="conn-banner__title">{{ fetchModelsResult.ok ? "已获取" : "获取失败" }}</div>
        <div class="conn-banner__hint">
          {{ fetchModelsResult.ok ? `已添加 ${fetchModelsResult.models.length} 个远端模型。` : fetchModelsResult.error }}
        </div>
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

    <div v-if="modelPool().length" class="settings-model-pool" data-agent-id="settings.assistant-ai.model-pool">
      <div
        v-for="item in modelPool()"
        :key="item.id"
        class="settings-row"
      >
        <div class="settings-row__label">{{ item.id }}</div>
        <input
          type="text"
          class="ui-input"
          placeholder="显示名"
          :aria-label="`${item.id} 显示名`"
          :value="item.label"
          @input="(e) => updateModelLabel(item.id, (e.target as HTMLInputElement).value)"
        />
      </div>
    </div>
    <p v-else class="muted" style="margin: 8px 0 0;">暂无备选模型</p>
  </div>
</template>
