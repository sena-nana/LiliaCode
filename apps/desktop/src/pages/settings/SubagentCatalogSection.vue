<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import Bot from "@lucide/vue/dist/esm/icons/bot.mjs";
import Pencil from "@lucide/vue/dist/esm/icons/pencil.mjs";
import Plus from "@lucide/vue/dist/esm/icons/plus.mjs";
import Trash2 from "@lucide/vue/dist/esm/icons/trash-2.mjs";
import type { CustomSubagentDefinition } from "@lilia/contracts";
import { useAgentInteractionSettings } from "../../composables/useAgentInteractionSettings";

interface DraftState {
  id: string | null;
  name: string;
  description: string;
  instruction: string;
  enabled: boolean;
}

const store = useAgentInteractionSettings();
const subagents = store.subagents;
const saving = ref(false);
const loading = ref(false);
const error = ref<string | null>(null);
const editing = ref(false);
const draft = ref<DraftState>(emptyDraft());
let disposed = false;

const hasSubagents = computed(() => subagents.value.length > 0);

function emptyDraft(): DraftState {
  return {
    id: null,
    name: "",
    description: "",
    instruction: "",
    enabled: true,
  };
}

function toSubagentInput(source: DraftState | CustomSubagentDefinition) {
  return {
    id: source.id,
    name: source.name,
    description: source.description,
    instruction: source.instruction,
    enabled: source.enabled,
  };
}

function startCreate() {
  draft.value = emptyDraft();
  editing.value = true;
  error.value = null;
}

function startEdit(item: CustomSubagentDefinition) {
  draft.value = {
    id: item.id,
    name: item.name,
    description: item.description,
    instruction: item.instruction,
    enabled: item.enabled,
  };
  editing.value = true;
  error.value = null;
}

function cancelEdit() {
  draft.value = emptyDraft();
  editing.value = false;
  error.value = null;
}

async function loadSubagents() {
  if (disposed) return;
  loading.value = true;
  try {
    await store.loadSubagents();
  } catch (err) {
    if (!disposed) error.value = `读取自定义 Agent 失败：${String(err)}`;
  } finally {
    if (!disposed) loading.value = false;
  }
}

async function saveDraft() {
  if (disposed) return;
  saving.value = true;
  error.value = null;
  try {
    await store.saveSubagent(toSubagentInput(draft.value));
    if (!disposed) cancelEdit();
  } catch (err) {
    if (!disposed) error.value = `保存自定义 Agent 失败：${String(err)}`;
  } finally {
    if (!disposed) saving.value = false;
  }
}

async function toggleEnabled(item: CustomSubagentDefinition) {
  if (disposed) return;
  saving.value = true;
  error.value = null;
  try {
    await store.saveSubagent({
      ...toSubagentInput(item),
      enabled: !item.enabled,
    });
  } catch (err) {
    if (!disposed) error.value = `更新自定义 Agent 状态失败：${String(err)}`;
  } finally {
    if (!disposed) saving.value = false;
  }
}

async function remove(item: CustomSubagentDefinition) {
  if (disposed) return;
  saving.value = true;
  error.value = null;
  try {
    await store.deleteSubagent(item.id);
    if (!disposed && draft.value.id === item.id) cancelEdit();
  } catch (err) {
    if (!disposed) error.value = `删除自定义 Agent 失败：${String(err)}`;
  } finally {
    if (!disposed) saving.value = false;
  }
}

onMounted(() => {
  disposed = false;
  void loadSubagents();
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
  <section class="card subagent-section" data-agent-id="settings.subagents">
    <div class="subagent-section__header">
      <div class="subagent-section__title">
        <Bot :size="14" aria-hidden="true" />
        <span>自定义 Agent</span>
      </div>
      <button type="button" class="ui-btn" data-agent-id="settings.subagents.create" :disabled="saving" @click="startCreate">
        <Plus :size="14" aria-hidden="true" />
        新建 Agent
      </button>
    </div>

    <div v-if="editing" class="subagent-editor">
      <label class="subagent-field">
        <span>名称</span>
        <input v-model="draft.name" type="text" data-agent-id="settings.subagents.form.name" placeholder="例如：Reviewer" />
      </label>
      <label class="subagent-field">
        <span>描述</span>
        <input v-model="draft.description" type="text" data-agent-id="settings.subagents.form.description" placeholder="一句话说明这个 Agent 适合做什么" />
      </label>
      <label class="subagent-field">
        <span>职责说明</span>
        <textarea
          v-model="draft.instruction"
          rows="5"
          data-agent-id="settings.subagents.form.instruction"
          placeholder="告诉 Agent 它的职责、边界和输出要求"
        />
      </label>
      <div class="subagent-editor__actions">
        <button type="button" class="ui-btn ui-btn--primary" data-agent-id="settings.subagents.form.save" :disabled="saving" @click="saveDraft">
          保存
        </button>
        <button type="button" class="ui-btn" data-agent-id="settings.subagents.form.cancel" :disabled="saving" @click="cancelEdit">
          取消
        </button>
      </div>
    </div>

    <div v-if="loading" class="subagent-empty">正在加载自定义 Agent…</div>
    <div v-else-if="!hasSubagents" class="subagent-empty">还没有自定义 Agent</div>
    <ul v-else class="subagent-list">
      <li v-for="item in subagents" :key="item.id" class="subagent-item">
        <div class="subagent-item__content">
          <div class="subagent-item__title">{{ item.name }}</div>
          <div class="subagent-item__meta">
            {{ item.enabled ? "已启用" : "已停用" }}
            <span v-if="item.description"> · {{ item.description }}</span>
          </div>
        </div>
        <div class="subagent-item__actions">
          <button type="button" class="ui-btn" :data-agent-id="`settings.subagents.item.${item.id}.edit`" :disabled="saving" @click="startEdit(item)">
            <Pencil :size="14" aria-hidden="true" />
            编辑
          </button>
          <button type="button" class="ui-btn" :data-agent-id="`settings.subagents.item.${item.id}.toggle`" :disabled="saving" @click="toggleEnabled(item)">
            {{ item.enabled ? "停用" : "启用" }}
          </button>
          <button type="button" class="ui-btn ui-btn--danger" :data-agent-id="`settings.subagents.item.${item.id}.remove`" :disabled="saving" @click="remove(item)">
            <Trash2 :size="14" aria-hidden="true" />
            删除
          </button>
        </div>
      </li>
    </ul>

    <div v-if="error" class="conn-banner conn-banner--err">
      <div>
        <div class="conn-banner__title">自定义 Agent</div>
        <div class="conn-banner__hint">{{ error }}</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.subagent-section {
  margin-top: 20px;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
  padding-top: 16px;
}

.subagent-section__header,
.subagent-item,
.subagent-editor__actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.subagent-section__title,
.subagent-item__actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.subagent-editor {
  display: grid;
  gap: 12px;
  margin-top: 14px;
  margin-bottom: 14px;
}

.subagent-field {
  display: grid;
  gap: 6px;
}

.subagent-field input,
.subagent-field textarea {
  width: 100%;
}

.subagent-list {
  list-style: none;
  margin: 14px 0 0;
  padding: 0;
  display: grid;
  gap: 10px;
}

.subagent-item {
  border: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
  border-radius: 12px;
  padding: 12px;
}

.subagent-item__content {
  min-width: 0;
}

.subagent-item__title {
  font-weight: 600;
}

.subagent-item__meta,
.subagent-empty {
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
  font-size: 13px;
}
</style>

