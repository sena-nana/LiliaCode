<script setup lang="ts">
import { nextTick, ref } from "vue";
import { FolderPlus } from "lucide-vue-next";
import type { Project } from "@lilia/contracts";
import { createProject } from "../../services/projectsStore";

defineProps<{ open: boolean }>();
const emit = defineEmits<{
  close: [];
  created: [project: Project];
}>();

const categoryName = ref("");
const categoryInput = ref<HTMLInputElement | null>(null);

async function init() {
  categoryName.value = "";
  await nextTick();
  categoryInput.value?.focus();
}

function confirm() {
  const name = categoryName.value.trim();
  if (!name) return;
  const project = createProject({ name, cwd: null });
  emit("created", project);
  emit("close");
}

defineExpose({ init });
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div v-if="open" class="search-palette" role="dialog" aria-modal="true" aria-label="创建空分类"
        @click.self="emit('close')">
        <div class="search-palette__card dialog__card">
          <div class="dialog__header">
            <FolderPlus :size="14" aria-hidden="true" />
            <span>创建空分类</span>
          </div>
          <div class="dialog__body">
            <label>
              <span>分类名称</span>
              <input ref="categoryInput" v-model="categoryName" type="text" class="text-input"
                placeholder="例如：实验、归档…"
                @keydown.enter.prevent="confirm" />
            </label>
            <p class="plugins-create__hint">
              空分类不绑定本地目录，只用来在侧栏里把收集箱里的对话归到一起。
            </p>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ghost" @click="emit('close')">取消</button>
            <button type="button" class="primary" :disabled="!categoryName.trim()" @click="confirm">
              创建
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
