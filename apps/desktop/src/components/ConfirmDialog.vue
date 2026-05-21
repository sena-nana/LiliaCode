<script setup lang="ts">
/**
 * 通用确认弹层。
 *
 * 视觉上复用全局命令面板风格的 backdrop / 进出过渡（`search-palette` 类名是
 * 历史命名，仅保留过渡 + 卡片几何）；内部用 `.dialog__*` 一套通用 class 摆
 * header / body / actions。
 * 删除类操作传 `danger=true`，主按钮换成红色 ghost.danger。
 *
 * 不做嵌套堆叠管理：调用方自己保证同一时刻只有一个 confirm 在弹。
 */
import { AlertTriangle } from "lucide-vue-next";

defineProps<{
  open: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  /** 危险动作（删除等）：图标 + 主按钮换红。 */
  danger?: boolean;
  /** 操作进行中：禁用按钮并把主按钮文案换成 busyText。 */
  busy?: boolean;
  busyText?: string;
}>();

const emit = defineEmits<{ confirm: []; cancel: [] }>();

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    emit("cancel");
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="search-palette">
      <div
        v-if="open"
        class="search-palette"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
        @click.self="emit('cancel')"
        @keydown="onKeydown"
      >
        <div class="search-palette__card dialog__card">
          <div class="dialog__header" :class="{ 'dialog__header--danger': danger }">
            <AlertTriangle v-if="danger" :size="14" aria-hidden="true" />
            <span>{{ title }}</span>
          </div>
          <div class="dialog__body">
            <p>{{ message }}</p>
          </div>
          <div class="dialog__actions">
            <button type="button" class="ghost" :disabled="busy" @click="emit('cancel')">
              {{ cancelText ?? "取消" }}
            </button>
            <button
              type="button"
              :class="danger ? 'ghost danger' : 'primary'"
              :disabled="busy"
              @click="emit('confirm')"
            >
              {{ busy ? (busyText ?? "处理中…") : (confirmText ?? "确认") }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
