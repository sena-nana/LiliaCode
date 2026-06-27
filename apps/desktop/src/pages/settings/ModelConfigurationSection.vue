<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { Brain, MessageSquarePlus, Save } from "lucide-vue-next";
import type {
  AssistantAIModelPoolItem,
  ModelFeatureSettings,
  ModelTier,
  SuggestionSettings,
} from "@lilia/contracts";
import { DEFAULT_SUGGESTION_SOURCE } from "@lilia/contracts";
import {
  getConversationSuggestionSettings,
  getModelFeatureSettings,
  listModelFeatureOptions,
  setConversationSuggestionSettings,
  setModelFeatureSettings,
} from "../../services/chat";

type FeatureKey = Exclude<keyof ModelFeatureSettings, "chat">;

const suggestionSettings = ref<SuggestionSettings>({
  enabled: true,
  source: DEFAULT_SUGGESTION_SOURCE,
});
const modelFeatureSettings = ref<ModelFeatureSettings>({
  chat: { light: null, normal: null, deep: null },
  title: null,
  suggestion: null,
  promptRouter: null,
  promptOptimize: null,
  autoTurnDecision: null,
});
const modelOptions = ref<AssistantAIModelPoolItem[]>([]);
const savingSuggestions = ref(false);
const savingModelFeatures = ref(false);
let disposed = false;

const chatRows: Array<{ tier: ModelTier; label: string }> = [
  { tier: "light", label: "主对话 Light" },
  { tier: "normal", label: "主对话 Normal" },
  { tier: "deep", label: "主对话 Deep" },
];
const featureRows: Array<{ key: FeatureKey; label: string }> = [
  { key: "title", label: "标题生成" },
  { key: "suggestion", label: "新对话建议" },
  { key: "promptRouter", label: "Prompt Router" },
  { key: "promptOptimize", label: "Prompt Optimize" },
  { key: "autoTurnDecision", label: "自动回合决策" },
];

const hasModelOptions = computed(() => modelOptions.value.length > 0);

async function loadAll() {
  try {
    const [suggestions, featureSettings, models] = await Promise.all([
      getConversationSuggestionSettings(),
      getModelFeatureSettings(),
      listModelFeatureOptions(),
    ]);
    if (disposed) return;
    suggestionSettings.value = suggestions;
    modelFeatureSettings.value = featureSettings;
    modelOptions.value = models;
  } catch (err) {
    console.error("[settings] load model configuration failed", err);
  }
}

async function setSuggestionEnabled(enabled: boolean) {
  if (disposed) return;
  const next: SuggestionSettings = { ...suggestionSettings.value, enabled };
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

function setChatTier(tier: ModelTier, value: string) {
  modelFeatureSettings.value = {
    ...modelFeatureSettings.value,
    chat: {
      ...modelFeatureSettings.value.chat,
      [tier]: value || null,
    },
  };
}

function setFeature(key: FeatureKey, value: string) {
  modelFeatureSettings.value = {
    ...modelFeatureSettings.value,
    [key]: value || null,
  };
}

async function saveModelFeatures() {
  if (disposed) return;
  savingModelFeatures.value = true;
  try {
    await setModelFeatureSettings(modelFeatureSettings.value);
  } catch (err) {
    console.error("[settings] save model feature settings failed", err);
  } finally {
    if (!disposed) savingModelFeatures.value = false;
  }
}

onMounted(() => {
  disposed = false;
  void loadAll();
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
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
          data-agent-id="settings.suggestions.enabled.on"
          :class="{ 'is-active': suggestionSettings.enabled }"
          :disabled="savingSuggestions"
          @click="setSuggestionEnabled(true)"
        >
          开启
        </button>
        <button
          type="button"
          role="radio"
          :aria-checked="!suggestionSettings.enabled"
          data-agent-id="settings.suggestions.enabled.off"
          :class="{ 'is-active': !suggestionSettings.enabled }"
          :disabled="savingSuggestions"
          @click="setSuggestionEnabled(false)"
        >
          关闭
        </button>
      </div>
    </div>
  </div>

  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Brain :size="14" aria-hidden="true" />
        模型分配
      </span>
      <button
        type="button"
        class="ui-button ui-button--ghost"
        data-agent-id="settings.model-features.save"
        :disabled="savingModelFeatures"
        @click="saveModelFeatures"
      >
        <Save :size="12" aria-hidden="true" />
        {{ savingModelFeatures ? "保存中…" : "保存" }}
      </button>
    </h2>

    <div v-for="row in chatRows" :key="row.tier" class="settings-row">
      <div class="settings-row__label">{{ row.label }}</div>
      <select
        class="ui-input"
        :aria-label="row.label"
        :value="modelFeatureSettings.chat[row.tier] ?? ''"
        :disabled="!hasModelOptions"
        @change="(e) => setChatTier(row.tier, (e.target as HTMLSelectElement).value)"
      >
        <option value="">默认</option>
        <option v-for="option in modelOptions" :key="option.id" :value="option.id">
          {{ option.label }} ({{ option.id }})
        </option>
      </select>
    </div>

    <div v-for="row in featureRows" :key="row.key" class="settings-row">
      <div class="settings-row__label">{{ row.label }}</div>
      <select
        class="ui-input"
        :aria-label="row.label"
        :value="modelFeatureSettings[row.key] ?? ''"
        :disabled="!hasModelOptions"
        @change="(e) => setFeature(row.key, (e.target as HTMLSelectElement).value)"
      >
        <option value="">默认</option>
        <option v-for="option in modelOptions" :key="option.id" :value="option.id">
          {{ option.label }} ({{ option.id }})
        </option>
      </select>
    </div>
  </div>
</template>
