<script setup lang="ts">
/**
 * 全局右键菜单宿主：监听 useContextMenu 的状态，Teleport 到 body。
 * 渲染前测一次尺寸，clamp 在视口内避免穿底/穿右。
 */
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  finalizeClosedContextMenu,
  isContextMenuItemPending,
  selectContextMenuItem,
  useContextMenu,
  type ContextMenuItem,
} from "../composables/useContextMenu";
import {
  clampAnchoredMenuPosition,
  createAnchoredMenuPosition,
  resolveMenuTransformOrigin,
  SB_MENU_POP_TRANSITION_MS,
} from "../composables/menuMotion";

const { state } = useContextMenu();

const menuEl = ref<HTMLElement | null>(null);
const rendered = ref(false);
const pos = ref(createAnchoredMenuPosition(0, 0));
const origin = ref({ x: 0, y: 0 });
let geometrySeq = 0;
let disposed = false;

function displayLabel(item: ContextMenuItem) {
  return isContextMenuItemPending(item) ? item.confirmLabel : item.label;
}

function isDanger(item: ContextMenuItem) {
  return item.danger || isContextMenuItemPending(item);
}

async function updateGeometry() {
  const seq = ++geometrySeq;
  const openSeq = state.openSeq;
  const initialPos = createAnchoredMenuPosition(
    state.x,
    state.y,
    state.anchorX,
    state.anchorY,
  );
  pos.value = initialPos;
  origin.value = resolveMenuTransformOrigin(initialPos);
  await nextTick();
  if (disposed || seq !== geometrySeq || !state.open || state.openSeq !== openSeq) return;
  const el = menuEl.value;
  if (!el) return;
  const clampedPos = clampAnchoredMenuPosition(initialPos, el.offsetWidth, el.offsetHeight);
  pos.value = clampedPos;
  origin.value = resolveMenuTransformOrigin(clampedPos, el.offsetWidth, el.offsetHeight);
}

function onAfterLeave() {
  finalizeClosedContextMenu();
}

watch(
  () => [state.openSeq, state.open] as const,
  ([, open]) => {
    geometrySeq += 1;
    if (!open) {
      rendered.value = false;
      return;
    }
    rendered.value = true;
    void updateGeometry();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  disposed = true;
  geometrySeq += 1;
});
</script>

<template>
  <Teleport to="body">
    <Transition
      name="sb-menu-pop"
      :duration="SB_MENU_POP_TRANSITION_MS"
      @after-leave="onAfterLeave"
    >
      <div
        v-if="rendered"
        ref="menuEl"
        class="ctx-menu sb-menu"
        role="menu"
        :style="{
          left: `${pos.x}px`,
          top: `${pos.y}px`,
          '--sb-menu-origin-x': `${origin.x}px`,
          '--sb-menu-origin-y': `${origin.y}px`,
        }"
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
    </Transition>
  </Teleport>
</template>
