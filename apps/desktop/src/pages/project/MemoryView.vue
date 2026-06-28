<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { Check, Pencil, Plus, RotateCcw, Save, Trash2 } from "@lucide/vue";
import {
  DEFAULT_MEMORY_SETTINGS,
  MEMORY_SCOPE_DISPLAY_SPECS,
  createMemoryUpsertInput,
  normalizeMemoryCooldownTurns,
  normalizeMemorySettings,
  type Memory,
  type MemoryScope,
  type MemorySettings,
} from "@lilia/contracts";
import {
  deleteMemory,
  getMemorySettings,
  listMemories,
  setMemoryEnabled,
  setMemorySettings,
  upsertMemory,
} from "../../services/memory";

const props = defineProps<{ projectId: string }>();

type FormState = {
  id: string | null;
  scope: MemoryScope;
  title: string;
  body: string;
  tags: string;
  enabled: boolean;
};

const memories = ref<Memory[]>([]);
const settings = reactive<MemorySettings>({ ...DEFAULT_MEMORY_SETTINGS });
const loading = ref(false);
const saving = ref(false);
const settingsSaving = ref(false);
const errorMessage = ref("");
let disposed = false;
let loadSeq = 0;

const form = reactive<FormState>({
  id: null,
  scope: "project",
  title: "",
  body: "",
  tags: "",
  enabled: true,
});

const memorySections = computed(() =>
  MEMORY_SCOPE_DISPLAY_SPECS.map((section) => ({
    ...section,
    items: memories.value.filter((memory) => memory.scope === section.scope),
  })),
);
const isEditing = computed(() => Boolean(form.id));
const activeMemory = computed(() =>
  form.id ? memories.value.find((memory) => memory.id === form.id) ?? null : null,
);

function resetForm(scope: MemoryScope = "project") {
  form.id = null;
  form.scope = scope;
  form.title = "";
  form.body = "";
  form.tags = "";
  form.enabled = true;
}

function editMemory(memory: Memory) {
  form.id = memory.id;
  form.scope = memory.scope;
  form.title = memory.title;
  form.body = memory.body;
  form.tags = memory.tags.join(", ");
  form.enabled = memory.enabled;
}

function formatTime(timestamp: number): string {
  if (!timestamp) return "未记录";
  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

async function loadMemoryView() {
  if (disposed) return;
  const seq = ++loadSeq;
  loading.value = true;
  errorMessage.value = "";
  try {
    const [items, nextSettings] = await Promise.all([
      listMemories(props.projectId),
      getMemorySettings(),
    ]);
    if (disposed || seq !== loadSeq) return;
    memories.value = items;
    Object.assign(settings, normalizeMemorySettings(nextSettings));
  } catch (err) {
    if (!disposed && seq === loadSeq) {
      errorMessage.value = err instanceof Error ? err.message : String(err);
    }
  } finally {
    if (!disposed && seq === loadSeq) loading.value = false;
  }
}

async function saveSettings() {
  if (disposed) return;
  settingsSaving.value = true;
  errorMessage.value = "";
  try {
    settings.cooldownTurns = normalizeMemoryCooldownTurns(settings.cooldownTurns);
    await setMemorySettings({ ...settings });
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (!disposed) settingsSaving.value = false;
  }
}

async function saveMemory() {
  const title = form.title.trim();
  const body = form.body.trim();
  if (disposed || !title || !body || saving.value) return;
  saving.value = true;
  errorMessage.value = "";
  try {
    const input = createMemoryUpsertInput({
      id: form.id,
      scope: form.scope,
      projectId: props.projectId,
      title,
      body,
      tags: form.tags,
      enabled: form.enabled,
      sourceTaskId: activeMemory.value?.sourceTaskId ?? null,
    });
    const saved = await upsertMemory(input);
    if (disposed) return;
    memories.value = memories.value.some((memory) => memory.id === saved.id)
      ? memories.value.map((memory) => memory.id === saved.id ? saved : memory)
      : [saved, ...memories.value];
    editMemory(saved);
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (!disposed) saving.value = false;
  }
}

async function toggleMemory(memory: Memory) {
  if (disposed) return;
  errorMessage.value = "";
  try {
    const saved = await setMemoryEnabled(memory.id, !memory.enabled);
    if (disposed) return;
    memories.value = memories.value.map((item) => item.id === saved.id ? saved : item);
    if (form.id === saved.id) editMemory(saved);
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  }
}

async function removeMemory(memory: Memory) {
  if (disposed) return;
  errorMessage.value = "";
  try {
    const deleted = await deleteMemory(memory.id);
    if (disposed) return;
    if (deleted) {
      memories.value = memories.value.filter((item) => item.id !== memory.id);
      if (form.id === memory.id) resetForm(memory.scope);
    }
  } catch (err) {
    if (!disposed) errorMessage.value = err instanceof Error ? err.message : String(err);
  }
}

function addMemory(scope: MemoryScope) {
  resetForm(scope);
}

watch(
  () => props.projectId,
  () => {
    resetForm("project");
    void loadMemoryView();
  },
);

onMounted(() => {
  disposed = false;
  void loadMemoryView();
});

onBeforeUnmount(() => {
  disposed = true;
  loadSeq += 1;
});
</script>

<template>
  <div class="memory-view" data-agent-id="memory.page">
    <header class="memory-view__toolbar">
      <div class="memory-view__switches" aria-label="Memory 设置">
        <label class="memory-view__toggle">
          <input
            v-model="settings.enabled"
            type="checkbox"
            data-agent-id="memory.settings.enabled"
            :disabled="settingsSaving"
            @change="saveSettings"
          >
          <span>Memory</span>
        </label>
        <label class="memory-view__toggle">
          <input
            v-model="settings.baselineInjectionEnabled"
            type="checkbox"
            data-agent-id="memory.settings.baseline-injection"
            :disabled="settingsSaving || !settings.enabled"
            @change="saveSettings"
          >
          <span>基线注入</span>
        </label>
        <label class="memory-view__cooldown">
          <span>冷却 turn</span>
          <input
            v-model.number="settings.cooldownTurns"
            type="number"
            min="1"
            max="100"
            data-agent-id="memory.settings.cooldown-turns"
            :disabled="settingsSaving"
            @change="saveSettings"
          >
        </label>
      </div>
      <button
        type="button"
        class="memory-view__icon-button"
        data-agent-id="memory.refresh"
        :disabled="loading"
        title="刷新"
        @click="loadMemoryView"
      >
        <RotateCcw :size="15" aria-hidden="true" />
      </button>
    </header>

    <p v-if="errorMessage" class="memory-view__error">{{ errorMessage }}</p>

    <main class="memory-view__grid">
      <section class="memory-view__lists" aria-label="记忆列表">
        <template v-for="section in memorySections" :key="section.scope">
          <div class="memory-view__section-head">
            <h2>{{ section.title }}</h2>
            <button type="button" class="memory-view__text-button" :data-agent-id="`memory.${section.scope}.add`" @click="addMemory(section.scope)">
              <Plus :size="14" aria-hidden="true" />
              <span>新增</span>
            </button>
          </div>
          <div v-if="section.items.length" class="memory-view__list">
            <article
              v-for="memory in section.items"
              :key="memory.id"
              class="memory-view__item"
              :class="{ 'is-active': form.id === memory.id, 'is-disabled': !memory.enabled }"
            >
              <button type="button" class="memory-view__item-main" :data-agent-id="`memory.item.${memory.id}.open`" @click="editMemory(memory)">
                <span class="memory-view__item-title">{{ memory.title }}</span>
                <span class="memory-view__item-body">{{ memory.body }}</span>
                <span class="memory-view__item-meta">
                  {{ memory.enabled ? "启用" : "停用" }} · {{ formatTime(memory.updatedAt) }}
                </span>
              </button>
              <div class="memory-view__item-actions">
                <button type="button" title="编辑" :data-agent-id="`memory.item.${memory.id}.edit`" @click="editMemory(memory)">
                  <Pencil :size="14" aria-hidden="true" />
                </button>
                <button type="button" :title="memory.enabled ? '停用' : '启用'" :data-agent-id="`memory.item.${memory.id}.toggle`" @click="toggleMemory(memory)">
                  <Check :size="14" aria-hidden="true" />
                </button>
                <button type="button" title="删除" :data-agent-id="`memory.item.${memory.id}.delete`" @click="removeMemory(memory)">
                  <Trash2 :size="14" aria-hidden="true" />
                </button>
              </div>
            </article>
          </div>
          <p v-else class="memory-view__empty">{{ section.empty }}</p>
        </template>
      </section>

      <form class="memory-view__editor" @submit.prevent="saveMemory">
        <div class="memory-view__editor-head">
          <h2>{{ isEditing ? "编辑记忆" : "新增记忆" }}</h2>
          <div class="memory-view__scope">
            <label>
              <input v-model="form.scope" type="radio" value="project" data-agent-id="memory.form.scope.project">
              <span>项目</span>
            </label>
            <label>
              <input v-model="form.scope" type="radio" value="user" data-agent-id="memory.form.scope.user">
              <span>用户</span>
            </label>
          </div>
        </div>

        <label class="memory-view__field">
          <span>标题</span>
          <input v-model="form.title" type="text" maxlength="120" required data-agent-id="memory.form.title">
        </label>
        <label class="memory-view__field">
          <span>正文</span>
          <textarea v-model="form.body" rows="8" required data-agent-id="memory.form.body" />
        </label>
        <label class="memory-view__field">
          <span>标签</span>
          <input v-model="form.tags" type="text" placeholder="逗号分隔" data-agent-id="memory.form.tags">
        </label>
        <label class="memory-view__toggle memory-view__toggle--standalone">
          <input v-model="form.enabled" type="checkbox" data-agent-id="memory.form.enabled">
          <span>启用这条记忆</span>
        </label>

        <div class="memory-view__editor-actions">
          <button
            type="button"
            class="memory-view__text-button"
            data-agent-id="memory.form.clear"
            :disabled="saving"
            @click="resetForm('project')"
          >
            <Plus :size="14" aria-hidden="true" />
            <span>清空</span>
          </button>
          <button
            type="submit"
            class="memory-view__primary"
            data-agent-id="memory.form.save"
            :disabled="saving || !form.title.trim() || !form.body.trim()"
          >
            <Save :size="15" aria-hidden="true" />
            <span>{{ saving ? "保存中" : "保存" }}</span>
          </button>
        </div>
      </form>
    </main>
  </div>
</template>

<style scoped>
.memory-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 100%;
  padding: 16px 20px 20px;
  color: var(--text);
}

.memory-view__toolbar,
.memory-view__section-head,
.memory-view__editor-head,
.memory-view__editor-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.memory-view__toolbar {
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border);
}

.memory-view__switches,
.memory-view__scope,
.memory-view__item-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.memory-view__toggle,
.memory-view__cooldown,
.memory-view__scope label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
  font-size: 12px;
}

.memory-view__toggle input,
.memory-view__scope input {
  accent-color: var(--accent);
}

.memory-view__cooldown input {
  width: 64px;
  height: 26px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-elev);
  color: var(--text);
}

.memory-view__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 380px);
  gap: 16px;
  min-height: 0;
}

.memory-view__lists,
.memory-view__editor {
  min-width: 0;
}

.memory-view__section-head {
  margin: 4px 0 8px;
}

.memory-view__section-head h2,
.memory-view__editor h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.memory-view__list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 18px;
}

.memory-view__item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
}

.memory-view__item.is-active {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
}

.memory-view__item.is-disabled {
  opacity: 0.62;
}

.memory-view__item-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  text-align: left;
}

.memory-view__item-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
}

.memory-view__item-body {
  display: -webkit-box;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 18px;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.memory-view__item-meta,
.memory-view__empty {
  color: var(--text-muted);
  font-size: 11px;
}

.memory-view__empty {
  padding: 14px 0 20px;
}

.memory-view__item-actions button,
.memory-view__icon-button,
.memory-view__text-button,
.memory-view__primary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-subtle);
  color: var(--text);
}

.memory-view__item-actions button,
.memory-view__icon-button {
  width: 28px;
  height: 28px;
  padding: 0;
}

.memory-view__text-button,
.memory-view__primary {
  height: 30px;
  padding: 0 10px;
  font-size: 12px;
}

.memory-view__primary {
  border-color: color-mix(in srgb, var(--accent) 58%, var(--border));
  background: color-mix(in srgb, var(--accent) 22%, var(--bg-elev));
}

.memory-view__item-actions button:hover,
.memory-view__icon-button:hover,
.memory-view__text-button:hover,
.memory-view__primary:hover {
  background: var(--bg-hover);
}

.memory-view__editor {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  align-self: start;
}

.memory-view__field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--text-muted);
  font-size: 12px;
}

.memory-view__field input,
.memory-view__field textarea {
  width: 100%;
  min-width: 0;
  padding: 8px 9px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg);
  color: var(--text);
  font: inherit;
}

.memory-view__field textarea {
  min-height: 148px;
  resize: vertical;
  line-height: 18px;
}

.memory-view__toggle--standalone {
  align-self: flex-start;
}

.memory-view__error {
  margin: 0;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, var(--err) 35%, var(--border));
  border-radius: 6px;
  background: var(--err-soft);
  color: var(--err);
  font-size: 12px;
}

button:disabled,
input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

@media (max-width: 920px) {
  .memory-view__grid {
    grid-template-columns: 1fr;
  }

  .memory-view__toolbar {
    align-items: flex-start;
  }

  .memory-view__switches {
    flex-wrap: wrap;
  }
}
</style>

