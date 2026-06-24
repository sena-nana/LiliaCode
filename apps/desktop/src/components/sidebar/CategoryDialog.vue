<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { FolderPlus } from "lucide-vue-next";

const emit = defineEmits<{
  close: [];
  confirm: [name: string];
}>();

const categoryName = ref("");
const categoryInput = ref<HTMLInputElement | null>(null);
let disposed = false;

async function init() {
  if (disposed) return;
  categoryName.value = "";
  await nextTick();
  if (disposed) return;
  categoryInput.value?.focus();
}

async function confirm() {
  if (disposed) return;
  const name = categoryName.value.trim();
  if (!name) return;
  emit("confirm", name);
  emit("close");
}

onMounted(() => {
  disposed = false;
  void init();
});

onBeforeUnmount(() => {
  disposed = true;
});
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        class="search-palette"
        role="dialog"
        aria-modal="true"
        aria-label="创建空分类"
        data-agent-id="sidebar.category-dialog"
        @click.self="emit('close')">
        <div class="search-palette__card dialog__card">
          <div class="dialog__header">
            <FolderPlus :size="14" aria-hidden="true" />
            <span>创建空分类</span>
          </div>
          <div class="dialog__body">
            <label>
              <span>分类名称</span>
              <input
                ref="categoryInput"
                v-model="categoryName"
                type="text"
                class="ui-input"
                data-agent-id="sidebar.category-dialog.name"
                placeholder="例如：实验、归档…"
                @keydown.enter.prevent="confirm" />
            </label>
            <p class="plugins-create__hint">
              空分类不绑定本地目录，只用来在侧栏里把收集箱里的对话归到一起。
            </p>
          </div>
          <div class="dialog__actions">
            <button
              type="button"
              class="ui-button ui-button--ghost"
              data-agent-id="sidebar.category-dialog.cancel"
              @click="emit('close')"
            >取消</button>
            <button
              type="button"
              class="ui-button ui-button--primary"
              data-agent-id="sidebar.category-dialog.create"
              :disabled="!categoryName.trim()"
              @click="confirm"
            >
              创建
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
