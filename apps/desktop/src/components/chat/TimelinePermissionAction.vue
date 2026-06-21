<script setup lang="ts">
withDefaults(defineProps<{
  autoDecisionText?: string;
  cwd: string;
  disabled?: boolean;
  permissionJson: string;
  reason: string;
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
    class="timeline-pending-action timeline-pending-action--codex"
    role="region"
    aria-label="权限确认"
  >
    <div class="timeline-pending-action__stack">
      <div class="timeline-pending-action__title-preview">
        {{ reason }}
      </div>
      <div class="timeline-pending-action__meta">{{ cwd }}</div>
      <pre class="timeline-code-block">{{ permissionJson }}</pre>
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
          同意
        </button>
      </div>
    </div>
  </section>
</template>
