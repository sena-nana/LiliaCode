<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { CodexComposerSettings, CodexPermissionProfile } from "@lilia/contracts";

const props = defineProps<{
  value?: CodexComposerSettings | null;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  update: [patch: CodexComposerSettings];
}>();

const metadataDraft = ref("");
const initialTurnsDraft = ref("");
const additionalContextDraft = ref("");
const excludeTurnsDraft = ref("");
const persistExtendedHistoryDraft = ref<"inherit" | "true" | "false">("inherit");
const commandExecPermissionDraft = ref<CodexPermissionProfile | "inherit">("inherit");
const error = ref<string | null>(null);

const commandPermissionOptions: Array<{ value: CodexPermissionProfile | "inherit"; label: string }> = [
  { value: "inherit", label: "继承" },
  { value: "default", label: "默认" },
  { value: "readOnly", label: "只读" },
  { value: "workspaceWrite", label: "工作区" },
  { value: "dangerFullAccess", label: "完全访问" },
];

const hasAdvancedValue = computed(() => {
  const value = props.value;
  return Boolean(
    value?.responsesApiClientMetadata ||
      value?.initialTurnsPage ||
      value?.additionalContext ||
      value?.persistExtendedHistory !== null && value?.persistExtendedHistory !== undefined ||
      value?.excludeTurns?.length ||
      value?.commandExecPermissionProfile,
  );
});

watch(
  () => props.value,
  (value) => {
    metadataDraft.value = stringifyJson(value?.responsesApiClientMetadata ?? null);
    initialTurnsDraft.value = stringifyJson(value?.initialTurnsPage ?? null);
    additionalContextDraft.value = value?.additionalContext ?? "";
    excludeTurnsDraft.value = (value?.excludeTurns ?? []).join("\n");
    persistExtendedHistoryDraft.value = value?.persistExtendedHistory === true
      ? "true"
      : value?.persistExtendedHistory === false
        ? "false"
        : "inherit";
    commandExecPermissionDraft.value = value?.commandExecPermissionProfile ?? "inherit";
    error.value = null;
  },
  { immediate: true, deep: true },
);

function stringifyJson(value: Record<string, unknown> | null): string {
  return value ? JSON.stringify(value, null, 2) : "";
}

function parseJsonObject(label: string, text: string): Record<string, unknown> | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    throw new Error(`${label} 需要是有效 JSON 对象`);
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`${label} 需要是 JSON 对象`);
  }
  if (Object.keys(parsed as Record<string, unknown>).length === 0) return null;
  return parsed as Record<string, unknown>;
}

function stringList(text: string): string[] {
  return Array.from(new Set(
    text
      .split(/\r?\n/)
      .map((item) => item.trim())
      .filter(Boolean),
  ));
}

function emitUpdate() {
  if (props.disabled) return;
  try {
    const patch: CodexComposerSettings = {
      responsesApiClientMetadata: parseJsonObject("Responses metadata", metadataDraft.value),
      initialTurnsPage: parseJsonObject("Initial turns page", initialTurnsDraft.value),
      additionalContext: additionalContextDraft.value.trim() || null,
      persistExtendedHistory: persistExtendedHistoryDraft.value === "inherit"
        ? null
        : persistExtendedHistoryDraft.value === "true",
      excludeTurns: stringList(excludeTurnsDraft.value),
      commandExecPermissionProfile: commandExecPermissionDraft.value === "inherit"
        ? null
        : commandExecPermissionDraft.value,
    };
    error.value = null;
    emit("update", patch);
  } catch (err) {
    error.value = String(err instanceof Error ? err.message : err);
  }
}
</script>

<template>
  <div class="codex-advanced-settings" :class="{ 'has-value': hasAdvancedValue }">
    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">Responses metadata</div>
      <textarea
        v-model="metadataDraft"
        class="ui-input ui-textarea"
        rows="3"
        placeholder='{"surface":"lilia"}'
        :disabled="disabled"
        @blur="emitUpdate"
      />
    </div>

    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">额外上下文</div>
      <textarea
        v-model="additionalContextDraft"
        class="ui-input ui-textarea"
        rows="3"
        placeholder="发送给 Codex 的本轮附加上下文"
        :disabled="disabled"
        @blur="emitUpdate"
      />
    </div>

    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">Initial turns page</div>
      <textarea
        v-model="initialTurnsDraft"
        class="ui-input ui-textarea"
        rows="3"
        placeholder='{"limit":20}'
        :disabled="disabled"
        @blur="emitUpdate"
      />
    </div>

    <div class="settings-row settings-row--stacked">
      <div class="settings-row__label">排除 turns</div>
      <textarea
        v-model="excludeTurnsDraft"
        class="ui-input ui-textarea"
        rows="3"
        placeholder="一行一个 turn id"
        :disabled="disabled"
        @blur="emitUpdate"
      />
    </div>

    <div class="settings-row">
      <div class="settings-row__label">扩展历史</div>
      <div class="ui-segmented" role="radiogroup" aria-label="Codex 扩展历史">
        <button
          type="button"
          role="radio"
          :aria-checked="persistExtendedHistoryDraft === 'inherit'"
          :class="{ 'is-active': persistExtendedHistoryDraft === 'inherit' }"
          :disabled="disabled"
          @click="persistExtendedHistoryDraft = 'inherit'; emitUpdate()"
        >继承</button>
        <button
          type="button"
          role="radio"
          :aria-checked="persistExtendedHistoryDraft === 'true'"
          :class="{ 'is-active': persistExtendedHistoryDraft === 'true' }"
          :disabled="disabled"
          @click="persistExtendedHistoryDraft = 'true'; emitUpdate()"
        >开启</button>
        <button
          type="button"
          role="radio"
          :aria-checked="persistExtendedHistoryDraft === 'false'"
          :class="{ 'is-active': persistExtendedHistoryDraft === 'false' }"
          :disabled="disabled"
          @click="persistExtendedHistoryDraft = 'false'; emitUpdate()"
        >关闭</button>
      </div>
    </div>

    <div class="settings-row">
      <div class="settings-row__label">编辑命令权限</div>
      <select
        v-model="commandExecPermissionDraft"
        class="ui-input"
        :disabled="disabled"
        aria-label="Codex 编辑命令权限"
        @change="emitUpdate"
      >
        <option v-for="option in commandPermissionOptions" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
    </div>

    <div v-if="error" class="settings-inline-error">{{ error }}</div>
  </div>
</template>
