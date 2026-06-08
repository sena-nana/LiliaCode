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
