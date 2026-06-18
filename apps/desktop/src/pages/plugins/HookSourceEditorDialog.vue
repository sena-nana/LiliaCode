<script setup lang="ts">
import { Plus, Trash2, Workflow } from "lucide-vue-next";
import type { HookHandlerDraftRow } from "./useHookSourceEditor";

defineProps<{
  open: boolean;
  title: string;
  sourceName: string;
  sourcePath: string | null;
  handlerRows: HookHandlerDraftRow[];
  saving: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  "add-handler": [];
  "remove-handler": [index: number];
  confirm: [];
}>();
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        v-if="open"
        class="search-palette"
        role="dialog"
        aria-modal="true"
        aria-label="Hooks 编辑器"
        @click.self="emit('update:open', false)"
      >
        <div class="search-palette__card dialog__card plugins-hook-editor">
          <div class="dialog__header">
            <Workflow :size="14" aria-hidden="true" />
            <span>{{ title }}</span>
          </div>
          <div class="dialog__body">
            <p class="plugins-create__hint">
              正在编辑 <strong>{{ sourceName }}</strong>
              <template v-if="sourcePath">
                ，保存到 <code>{{ sourcePath }}</code>
              </template>
              。
            </p>
            <div class="plugins-hook-editor__head">
              <span>Handlers</span>
              <button type="button" class="ui-button ui-button--ghost" @click="emit('add-handler')">
                <Plus :size="12" aria-hidden="true" /> 添加 Handler
              </button>
            </div>
            <div
              v-for="(row, index) in handlerRows"
              :key="row.id || `${index}:${row.event}:${row.matcher}`"
              class="plugins-hook-editor__card"
            >
              <div class="plugins-hook-editor__card-head">
                <strong>Handler {{ index + 1 }}</strong>
                <button
                  type="button"
                  class="ui-button ui-button--ghost"
                  aria-label="删除 Handler"
                  @click="emit('remove-handler', index)"
                >
                  <Trash2 :size="12" aria-hidden="true" />
                </button>
              </div>
              <div class="plugins-hook-editor__grid">
                <label>
                  <span>Event</span>
                  <input v-model="row.event" type="text" class="ui-input" placeholder="PostToolUse" />
                </label>
                <label>
                  <span>Matcher</span>
                  <input v-model="row.matcher" type="text" class="ui-input" placeholder="Bash" />
                </label>
                <label>
                  <span>Type</span>
                  <input v-model="row.type" type="text" class="ui-input" placeholder="command" />
                </label>
                <label>
                  <span>Timeout</span>
                  <input v-model="row.timeoutSeconds" type="text" class="ui-input" placeholder="30" />
                </label>
                <label>
                  <span>Command</span>
                  <input v-model="row.command" type="text" class="ui-input" placeholder="node hook.js" />
                </label>
                <label>
                  <span>Windows Command</span>
                  <input v-model="row.commandWindows" type="text" class="ui-input" placeholder="powershell -File hook.ps1" />
                </label>
                <label class="plugins-hook-editor__field-wide">
                  <span>Status Message</span>
                  <input v-model="row.statusMessage" type="text" class="ui-input" placeholder="Running hook…" />
                </label>
                <label class="plugins-hook-editor__field-wide">
                  <span>Group JSON</span>
                  <textarea
                    v-model="row.groupAdvancedJson"
                    class="ui-input"
                    rows="4"
                    placeholder='{"share": "group fields"}'
                  />
                </label>
                <label class="plugins-hook-editor__field-wide">
                  <span>Handler JSON</span>
                  <textarea
                    v-model="row.advancedJson"
                    class="ui-input"
                    rows="5"
                    placeholder='{"env": {"FOO": "bar"}}'
                  />
                </label>
              </div>
            </div>
            <p v-if="error" class="plugins-create__error">{{ error }}</p>
          </div>
          <div class="dialog__actions">
            <button
              type="button"
              class="ui-button ui-button--ghost"
              :disabled="saving"
              @click="emit('update:open', false)"
            >
              取消
            </button>
            <button type="button" class="ui-button ui-button--primary" :disabled="saving" @click="emit('confirm')">
              {{ saving ? "保存中…" : "保存" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
