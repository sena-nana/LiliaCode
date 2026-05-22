<script setup lang="ts">
/**
 * 全局右键菜单宿主：监听 useContextMenu 的状态，Teleport 到 body。
 * 渲染前测一次尺寸，clamp 在视口内避免穿底/穿右。
 */
import { nextTick, ref, watch } from "vue";
import {
  isContextMenuItemPending,
  selectContextMenuItem,
  useContextMenu,
  type ContextMenuItem,
} from "../composables/useContextMenu";

const { state } = useContextMenu();

const menuEl = ref<HTMLElement | null>(null);
const pos = ref<{ x: number; y: number }>({ x: 0, y: 0 });

function displayLabel(item: ContextMenuItem) {
  return isContextMenuItemPending(item) ? item.confirmLabel : item.label;
}

function isDanger(item: ContextMenuItem) {
  return item.danger || isContextMenuItemPending(item);
}

watch(
  () => state.open,
  async (open) => {
    if (!open) return;
    // 先用锚点占位，下一帧拿到真实尺寸再 clamp，避免菜单先闪到边外再跳回来。
    pos.value = { x: state.x, y: state.y };
    await nextTick();
    const el = menuEl.value;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const x = Math.max(4, Math.min(state.x, window.innerWidth - w - 4));
    const y = Math.max(4, Math.min(state.y, window.innerHeight - h - 4));
    pos.value = { x, y };
  },
);
</script>

<template>
  <Teleport to="body">
    <div
      v-if="state.open"
      ref="menuEl"
      class="ctx-menu sb-menu"
      role="menu"
      :style="{ left: `${pos.x}px`, top: `${pos.y}px` }"
    >
      <button
        v-for="(item, i) in state.items"
        :key="item.id ?? i"
        type="button"
        class="sb-menu__item ctx-menu__item"
        :class="{
          'ctx-menu__item--danger': isDanger(item),
          'ctx-menu__item--pending': isContextMenuItemPending(item),
        }"
        :disabled="item.disabled"
        role="menuitem"
        @click="selectContextMenuItem(item)"
      >
        <component v-if="item.icon" :is="item.icon" :size="13" aria-hidden="true" />
        <span class="sb-menu__label">{{ displayLabel(item) }}</span>
      </button>
    </div>
  </Teleport>
</template>
