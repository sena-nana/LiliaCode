<script setup lang="ts">
import { ChevronDown, ChevronRight } from "@lucide/vue";
import { computed } from "vue";

const props = withDefaults(defineProps<{
  expanded: boolean;
  controlsId: string;
  toggleAgentId: string;
  expandLabel: string;
  collapseLabel: string;
  ariaLabel?: string;
  "aria-label"?: string;
  disabled?: boolean;
  withSwitch?: boolean;
}>(), {
  disabled: false,
  withSwitch: false,
});

const emit = defineEmits<{
  "update:expanded": [expanded: boolean];
}>();

function toggle() {
  if (props.disabled) return;
  emit("update:expanded", !props.expanded);
}

const resolvedAriaLabel = computed(() => props.ariaLabel ?? props["aria-label"] ?? "");
</script>

<template>
  <section class="card settings-collapsible-card" :aria-label="resolvedAriaLabel">
    <div
      class="settings-collapsible-card__header"
      :class="{ 'settings-collapsible-card__header--with-switch': withSwitch }"
      :data-agent-id="toggleAgentId"
      role="button"
      :tabindex="disabled ? -1 : 0"
      :aria-controls="controlsId"
      :aria-expanded="expanded"
      :aria-disabled="disabled || undefined"
      :aria-label="expanded ? collapseLabel : expandLabel"
      @click="toggle"
      @keydown.enter.prevent="toggle"
      @keydown.space.prevent="toggle"
    >
      <div class="settings-collapsible-card__summary">
        <slot name="summary" />
      </div>
      <div
        v-if="$slots.switch"
        class="settings-collapsible-card__switch"
        @click.stop
        @keydown.stop
      >
        <slot name="switch" />
      </div>
      <component
        :is="expanded ? ChevronDown : ChevronRight"
        :size="16"
        aria-hidden="true"
        class="settings-collapsible-card__chevron"
      />
    </div>

    <div v-if="expanded" :id="controlsId" class="settings-collapsible-card__details">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.settings-collapsible-card {
  overflow: hidden;
}

.settings-collapsible-card__header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 0;
  cursor: pointer;
}

.settings-collapsible-card__header--with-switch {
  grid-template-columns: minmax(0, 1fr) auto auto;
}

.settings-collapsible-card__header[aria-disabled="true"] {
  cursor: default;
}

.settings-collapsible-card__header:focus-visible {
  outline: 2px solid rgba(255, 255, 255, 0.18);
  outline-offset: -2px;
  border-radius: 14px;
}

.settings-collapsible-card__summary {
  min-width: 0;
}

.settings-collapsible-card__switch {
  justify-self: end;
}

.settings-collapsible-card__chevron {
  color: var(--text-secondary, rgba(255, 255, 255, 0.6));
}

.settings-collapsible-card__details {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--ui-border, rgba(255, 255, 255, 0.08));
}
</style>
