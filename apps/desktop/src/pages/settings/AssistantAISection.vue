<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { AlertTriangle, KeyRound, Plug, Plus, Save, Sparkles } from "@lucide/vue";
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

type ModelPoolSource = AssistantAIModelPoolItem["source"];

const MODEL_POOL_SOURCES: Array<{ key: ModelPoolSource; label: string }> = [
  { key: "remote", label: "远端" },
  { key: "legacy", label: "旧模型" },
];

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
const selectedModelSource = ref<ModelPoolSource>("remote");
const addingModel = ref(false);
const newModelId = ref("");
const newModelLabel = ref("");
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

const selectedModelPool = computed(() =>
  modelPool().filter((item) => item.source === selectedModelSource.value)
);

const sourceTabs = computed(() =>
  MODEL_POOL_SOURCES.map((source) => ({
    ...source,
    count: modelPool().filter((item) => item.source === source.key).length,
  }))
);

const selectedSourceLabel = computed(() =>
  sourceTabs.value.find((source) => source.key === selectedModelSource.value)?.label ?? "当前来源"
);

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

function cancelAddModel() {
  addingModel.value = false;
  newModelId.value = "";
  newModelLabel.value = "";
}

function addModelToPool() {
  const id = newModelId.value.trim();
  if (!id) return;
  const label = newModelLabel.value.trim() || id;
  const item: AssistantAIModelPoolItem = {
    id,
    label,
    source: selectedModelSource.value,
    backend: "codex",
  };
  const next = [...modelPool()];
  const index = next.findIndex((candidate) => candidate.id === id);
  if (index >= 0) next[index] = item;
  else next.push(item);
  assistantAIForm.value.modelPool = next;
  cancelAddModel();
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
  <div class="card assistant-ai-card">
    <div class="assistant-ai-card__header">
      <h2>
        <span class="card-h2__title">
          <Sparkles :size="14" aria-hidden="true" />
          模型池配置
        </span>
      </h2>
      <button
        type="button"
        class="ui-button ui-button--primary"
        data-agent-id="settings.assistant-ai.add-model"
        @click="addingModel = true"
      >
        <Plus :size="12" aria-hidden="true" />
        添加
      </button>
    </div>

    <div class="ui-tabs ui-tabs--pill assistant-ai-source-tabs" role="tablist" aria-label="模型来源">
      <button
        v-for="source in sourceTabs"
        :key="source.key"
        type="button"
        role="tab"
        class="ui-tabs__tab"
        :class="{ 'is-active': selectedModelSource === source.key }"
        :aria-selected="selectedModelSource === source.key"
        :data-agent-id="`settings.assistant-ai.source.${source.key}`"
        @click="selectedModelSource = source.key"
      >
        {{ source.label }}
        <span class="ui-tabs__count">{{ source.count }}</span>
      </button>
    </div>

    <div class="settings-model-pool" data-agent-id="settings.assistant-ai.model-pool">
      <div
        v-if="addingModel"
        class="settings-row settings-row--stacked assistant-ai-add-row"
        data-agent-id="settings.assistant-ai.add-model-row"
      >
        <div class="settings-row__label">添加{{ selectedSourceLabel }}模型</div>
        <div class="assistant-ai-add-row__fields">
          <input
            type="text"
            class="ui-input"
            aria-label="模型 ID"
            placeholder="model-id"
            data-agent-id="settings.assistant-ai.new-model-id"
            :value="newModelId"
            @input="(e) => (newModelId = (e.target as HTMLInputElement).value)"
            @keyup.enter="addModelToPool"
          />
          <input
            type="text"
            class="ui-input"
            aria-label="显示名"
            placeholder="显示名"
            data-agent-id="settings.assistant-ai.new-model-label"
            :value="newModelLabel"
            @input="(e) => (newModelLabel = (e.target as HTMLInputElement).value)"
            @keyup.enter="addModelToPool"
          />
          <button
            type="button"
            class="ui-button ui-button--primary"
            data-agent-id="settings.assistant-ai.confirm-add-model"
            :disabled="!newModelId.trim()"
            @click="addModelToPool"
          >
            <Plus :size="12" aria-hidden="true" />
            添加模型
          </button>
          <button
            type="button"
            class="ui-button ui-button--ghost"
            data-agent-id="settings.assistant-ai.cancel-add-model"
            @click="cancelAddModel"
          >
            取消
          </button>
        </div>
      </div>

      <div
        v-for="item in selectedModelPool"
        :key="item.id"
        class="settings-row"
      >
        <div class="settings-row__label">{{ item.id }}</div>
        <input
          type="text"
          class="ui-input"
          placeholder="显示名"
          :aria-label="`${item.id} 显示名`"
          :data-agent-id="`settings.assistant-ai.model-label.${item.id}`"
          :value="item.label"
          @input="(e) => updateModelLabel(item.id, (e.target as HTMLInputElement).value)"
        />
      </div>
      <p v-if="!selectedModelPool.length && !addingModel" class="muted assistant-ai-empty">
        当前来源暂无模型
      </p>
    </div>

    <div class="settings-row assistant-ai-connection-start">
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
  </div>
</template>

<style scoped>
.assistant-ai-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.assistant-ai-card__header h2 {
  margin: 0;
}

.assistant-ai-source-tabs {
  margin-bottom: 8px;
}

.assistant-ai-add-row__fields {
  display: grid;
  grid-template-columns: minmax(150px, 1fr) minmax(150px, 1fr) auto auto;
  gap: 8px;
  align-items: center;
}

.assistant-ai-add-row__fields .ui-input {
  width: 100%;
}

.assistant-ai-empty {
  margin: 10px 0 12px;
  font-size: 13px;
}

.assistant-ai-connection-start {
  border-top: 1px solid var(--border-soft);
  margin-top: 8px;
}

@media (max-width: 760px) {
  .assistant-ai-card__header {
    align-items: stretch;
    flex-direction: column;
  }

  .assistant-ai-card__header .ui-button {
    align-self: flex-start;
  }

  .assistant-ai-add-row__fields {
    grid-template-columns: 1fr;
  }
}
</style>

