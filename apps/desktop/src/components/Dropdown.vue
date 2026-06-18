<script setup lang="ts" generic="T extends string | number">
/**
 * 轻量级下拉：trigger（默认渲染 chip）+ popover。popover 默认向上展开
 * （composer 在底，向上更自然）。
 */

import { computed, onBeforeUnmount, ref, watch } from "vue";
import { ChevronDown } from "lucide-vue-next";
import { SB_MENU_POP_TRANSITION_MS } from "../composables/menuMotion";
import { useAnchoredMenuMotion } from "../composables/useAnchoredMenuMotion";

interface Option {
  value: T;
  label: string;
  hint?: string;
}

const props = defineProps<{
  modelValue: T;
  options: Option[];
  icon?: any;
  placeholder?: string;
  placement?: "top" | "bottom";
  disabled?: boolean;
}>();

const emit = defineEmits<{ "update:modelValue": [value: T] }>();

const open = ref(false);
const placement = computed(() => props.placement === "bottom" ? "bottom" : "top");
const {
  triggerEl,
  menuEl,
  overlayStyle,
  containsTarget,
  clearAnchor,
  captureAnchor,
  updateOrigin,
} = useAnchoredMenuMotion(open, placement);

const current = computed(() =>
  props.options.find((o) => o.value === props.modelValue),
);

const placementClass = computed(() =>
  props.placement === "bottom" ? "dd__menu--bottom" : "dd__menu--top",
);

function toggle(event: MouseEvent) {
  if (props.disabled) return;
  captureAnchor(event);
  open.value = !open.value;
}

function pick(opt: Option) {
  emit("update:modelValue", opt.value);
  open.value = false;
}

function onDocPointer(e: PointerEvent) {
  if (!containsTarget(e.target)) open.value = false;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value) {
    open.value = false;
    e.stopPropagation();
  }
}

watch(open, async (v) => {
  if (v) {
    await updateOrigin();
    document.addEventListener("pointerdown", onDocPointer, true);
    document.addEventListener("keydown", onKey);
  } else {
    clearAnchor();
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onKey);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointer, true);
  document.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div class="dd">
    <button
      ref="triggerEl"
      type="button"
      class="chat-chip"
      :class="{ 'is-open': open, 'is-disabled': disabled }"
      :disabled="disabled"
      :aria-haspopup="true"
      :aria-expanded="open"
      @click="toggle"
    >
      <component v-if="icon" :is="icon" :size="13" aria-hidden="true" />
      <span class="chat-chip__label">
        {{ current?.label ?? placeholder ?? "—" }}
      </span>
      <ChevronDown :size="12" aria-hidden="true" class="chat-chip__caret" />
    </button>

    <Teleport to="body">
      <Transition name="sb-menu-pop" :duration="SB_MENU_POP_TRANSITION_MS">
        <div
          v-if="open"
          ref="menuEl"
          class="dd__menu"
          :class="placementClass"
          role="listbox"
          :style="overlayStyle"
        >
          <button
            v-for="opt in options"
            :key="String(opt.value)"
            type="button"
            class="dd__item"
            :class="{ 'is-active': opt.value === modelValue }"
            role="option"
            :aria-selected="opt.value === modelValue"
            @click="pick(opt)"
          >
            <span class="dd__item-label">{{ opt.label }}</span>
            <span v-if="opt.hint" class="dd__item-hint">{{ opt.hint }}</span>
          </button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>
