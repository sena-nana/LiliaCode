<script setup lang="ts">
/** Roadmap 占位视图。 */
defineProps<{ projectId: string }>();
</script>

<template>
  <div class="roadmap-view">
    <div class="roadmap-view__empty">
      <h2>路线图</h2>
      <p>还没有里程碑。即将上线：垂直时间线 + 里程碑 CRUD + 未归类任务分组。</p>
    </div>
  </div>
</template>

<style scoped>
.roadmap-view {
  padding: 32px 20px;
}
.roadmap-view__empty {
  max-width: 480px;
  margin: 0 auto;
  text-align: center;
  color: var(--text-muted, #888);
}
.roadmap-view__empty h2 {
  margin: 0 0 12px;
  font-size: 16px;
  color: var(--text);
}
.roadmap-view__empty p {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
}
</style>
