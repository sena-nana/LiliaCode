<script setup lang="ts">
withDefaults(defineProps<{
  autoDecisionText?: string;
  proposedTitle: string;
}>(), {
  autoDecisionText: "",
});

const emit = defineEmits<{
  accept: [];
  decline: [];
}>();
</script>

<template>
  <section
    class="timeline-pending-action timeline-pending-action--title"
    role="region"
    aria-label="标题更新确认"
  >
    <span class="timeline-pending-action__title-preview">
      {{ proposedTitle }}
    </span>
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
        data-agent-id="timeline.title-update.decline"
        @click="emit('decline')"
      >
        忽略
      </button>
      <button
        type="button"
        class="ui-button ui-button--primary composer-inline__btn"
        data-agent-id="timeline.title-update.accept"
        @click="emit('accept')"
      >
        同意
      </button>
    </div>
  </section>
</template>
