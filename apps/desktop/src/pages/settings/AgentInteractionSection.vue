<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { AlertTriangle, Sparkles } from "lucide-vue-next";
import type {
  AgentInteractionSettings,
  CodexPermissionProfile,
  CodexProfileSettings,
  CodexReasoningEffort,
  CodexSettingsProfile,
} from "@lilia/contracts";
import { useAgentInteractionSettings } from "../../composables/useAgentInteractionSettings";

const agentInteractionStore = useAgentInteractionSettings();
const agentInteraction = agentInteractionStore.settings;
const savingAgentInteraction = ref(false);
const agentInteractionError = ref<string | null>(null);
const codexModelDraft = ref("");
const codexRootsDraft = ref("");

const profileOptions: Array<{ value: CodexSettingsProfile; label: string }> = [
  { value: "default", label: "默认" },
  { value: "fast", label: "快速" },
  { value: "balanced", label: "均衡" },
  { value: "deep", label: "深入" },
];

const effortOptions: Array<{ value: CodexReasoningEffort | null; label: string }> = [
  { value: null, label: "默认" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "极高" },
];

const codexPermissionOptions: Array<{ value: CodexPermissionProfile; label: string }> = [
  { value: "default", label: "跟随权限" },
  { value: "readOnly", label: "只读" },
  { value: "workspaceWrite", label: "工作区" },
  { value: "dangerFullAccess", label: "完全访问" },
];

const codexProfile = computed(() => agentInteraction.value.codexProfile);

function sameStringArray(a: readonly string[], b: readonly string[]) {
  return a.length === b.length && a.every((item, index) => item === b[index]);
}

function sameCodexProfile(a: unknown, b: unknown) {
  return JSON.stringify(a) === JSON.stringify(b);
}

watch(
  codexProfile,
  (next) => {
    codexModelDraft.value = next.model ?? "";
    codexRootsDraft.value = next.runtimeWorkspaceRoots.join("\n");
  },
  { immediate: true },
);

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

async function setCodexProfile(patch: Partial<CodexProfileSettings>) {
  await setAgentInteraction({
    codexProfile: {
      ...codexProfile.value,
      ...patch,
      runtimeWorkspaceRoots: [
        ...(patch.runtimeWorkspaceRoots ?? codexProfile.value.runtimeWorkspaceRoots),
      ],
      permissions: {
        ...codexProfile.value.permissions,
        ...(patch.permissions ?? {}),
      },
    },
  });
}

async function saveCodexModel() {
  const model = codexModelDraft.value.trim() || null;
  if (model === codexProfile.value.model) return;
  await setCodexProfile({ model });
}

async function saveCodexRoots() {
  const roots = Array.from(new Set(
    codexRootsDraft.value
      .split(/\r?\n/)
      .map((root) => root.trim())
      .filter(Boolean),
  ));
  if (sameStringArray(roots, codexProfile.value.runtimeWorkspaceRoots)) return;
  await setCodexProfile({ runtimeWorkspaceRoots: roots });
}

async function setAgentInteraction(patch: Partial<AgentInteractionSettings>) {
  const next = { ...agentInteraction.value, ...patch };
  if (
    next.nonInterruptMode === agentInteraction.value.nonInterruptMode &&
    next.debug === agentInteraction.value.debug &&
    sameCodexProfile(next.codexProfile, agentInteraction.value.codexProfile)
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

onMounted(loadAgentInteraction);
</script>

<template>
  <div class="card">
    <h2>
      <span class="card-h2__title">
        <Sparkles :size="14" aria-hidden="true" />
        Agent 交互
      </span>
    </h2>

    <div class="settings-row">
      <div class="settings-row__label">非打断模式</div>
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
      <div class="settings-row__label">Codex 配置档案</div>
      <div class="segmented" role="radiogroup" aria-label="Codex 配置档案">
        <button
          v-for="option in profileOptions"
          :key="option.value"
          type="button"
          role="radio"
          :aria-checked="codexProfile.profile === option.value"
          :class="{ 'is-active': codexProfile.profile === option.value }"
          :disabled="savingAgentInteraction"
          @click="setCodexProfile({ profile: option.value })"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">Codex 模型</div>
      <input
        v-model="codexModelDraft"
        class="settings-input"
        type="text"
        placeholder="默认 Codex 模型"
        :disabled="savingAgentInteraction"
        @blur="saveCodexModel"
      />
    </div>

    <div class="settings-row">
      <div class="settings-row__label">推理强度</div>
      <div class="segmented" role="radiogroup" aria-label="推理强度">
        <button
          v-for="option in effortOptions"
          :key="option.value ?? 'default'"
          type="button"
          role="radio"
          :aria-checked="codexProfile.reasoningEffort === option.value"
          :class="{ 'is-active': codexProfile.reasoningEffort === option.value }"
          :disabled="savingAgentInteraction"
          @click="setCodexProfile({ reasoningEffort: option.value })"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">运行时工作区根目录</div>
      <textarea
        v-model="codexRootsDraft"
        class="settings-input settings-input--textarea"
        placeholder="一行一个路径"
        rows="3"
        :disabled="savingAgentInteraction"
        @blur="saveCodexRoots"
      />
    </div>

    <div class="settings-row">
      <div class="settings-row__label">Codex 权限</div>
      <div class="segmented" role="radiogroup" aria-label="Codex 权限">
        <button
          v-for="option in codexPermissionOptions"
          :key="option.value"
          type="button"
          role="radio"
          :aria-checked="codexProfile.permissions.profile === option.value"
          :class="{ 'is-active': codexProfile.permissions.profile === option.value }"
          :disabled="savingAgentInteraction"
          @click="setCodexProfile({ permissions: { profile: option.value } })"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">Debug 面板</div>
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
</template>
