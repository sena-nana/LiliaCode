<script setup lang="ts">
/** Memory 占位视图。 */
defineProps<{ projectId: string }>();
</script>

<template>
  <div class="memory-view">
    <div class="memory-view__empty">
      <h2>记忆</h2>
      <p>还没有记忆。即将上线：用户级 / 项目级两层 + 对话内右键保存 + 发送时自动注入。</p>
    </div>
  </div>
</template>

<style scoped>
.memory-view {
  padding: 32px 20px;
}
.memory-view__empty {
  max-width: 480px;
  margin: 0 auto;
  text-align: center;
  color: var(--text-muted, #888);
}
.memory-view__empty h2 {
  margin: 0 0 12px;
  font-size: 16px;
  color: var(--text);
}
.memory-view__empty p {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
}
</style>
