<script setup lang="ts">
withDefaults(defineProps<{
  autoDecisionText?: string;
  disabled?: boolean;
  extraCount: number;
  reason: string;
  rows: Array<{ key: string; text: string }>;
}>(), {
  autoDecisionText: "",
  disabled: false,
});

const emit = defineEmits<{
  allow: [];
  deny: [];
}>();
</script>

<template>
  <section
    class="timeline-pending-action timeline-pending-action--architecture"
    role="region"
    aria-label="架构图变更确认"
  >
    <div class="timeline-pending-action__stack">
      <div class="timeline-pending-action__title-preview">
        {{ reason }}
      </div>
      <ul class="timeline-pending-action__mini-list">
        <li
          v-for="row in rows"
          :key="row.key"
        >
          {{ row.text }}
        </li>
        <li v-if="extraCount > 0">另有 {{ extraCount }} 项变更</li>
      </ul>
      <div
        v-if="autoDecisionText"
        class="composer-inline__auto-decision timeline-pending-action__auto-decision"
        role="status"
      >
        {{ autoDecisionText }}
      </div>
      <div class="composer-inline__actions">
        <button
          type="button"
          class="ui-button ui-button--ghost composer-inline__btn"
          :disabled="disabled"
          @click="emit('deny')"
        >
          拒绝
        </button>
        <button
          type="button"
          class="ui-button ui-button--primary composer-inline__btn"
          :disabled="disabled"
          @click="emit('allow')"
        >
          应用
        </button>
      </div>
    </div>
  </section>
</template>
