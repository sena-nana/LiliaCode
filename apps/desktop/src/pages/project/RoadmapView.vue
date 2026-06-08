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
