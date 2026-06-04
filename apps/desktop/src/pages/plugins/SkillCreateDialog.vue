<script setup lang="ts">
import { Sparkles } from "lucide-vue-next";

defineProps<{
  open: boolean;
  name: string;
  description: string;
  scopeHint: string;
  creating: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  "update:name": [value: string];
  "update:description": [value: string];
  confirm: [];
}>();
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        v-if="open"
        class="search-palette"
        role="dialog" aria-modal="true" aria-label="新建 Skill"
        @click.self="emit('update:open', false)"
      >
        <div class="search-palette__card dialog__card">
          <div class="dialog__header">
            <Sparkles :size="14" aria-hidden="true" />
            <span>新建 Claude Skill</span>
          </div>
          <div class="dialog__body">
            <label>
              <span>名称</span>
              <input
                :value="name" type="text"
                class="text-input"
                placeholder="kebab-case，仅 a-z 0-9 - _"
                @input="emit('update:name', ($event.target as HTMLInputElement).value)"
              />
            </label>
            <label>
              <span>描述</span>
              <textarea
                :value="description"
                class="text-input"
                rows="3"
                placeholder="一行描述，告诉 Claude 什么时候应该用这个 skill"
                @input="emit('update:description', ($event.target as HTMLTextAreaElement).value)"
              />
            </label>
            <p v-if="error" class="plugins-create__error">{{ error }}</p>
            <p class="plugins-create__hint">
              创建后会生成 <code>{{ scopeHint }}{{ name || '<name>' }}/SKILL.md</code>。
            </p>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ghost" :disabled="creating" @click="emit('update:open', false)">
              取消
            </button>
            <button type="button" class="primary" :disabled="creating" @click="emit('confirm')">
              {{ creating ? "创建中…" : "创建" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
