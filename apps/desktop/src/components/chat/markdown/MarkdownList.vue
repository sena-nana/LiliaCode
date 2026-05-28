<script setup lang="ts">
import MarkdownInline from "./MarkdownInline.vue";
import type { MarkdownListNode } from "./markdownParser";

defineProps<{
  list: MarkdownListNode;
}>();

function listStart(list: MarkdownListNode): number | undefined {
  return list.ordered && list.start !== null ? list.start : undefined;
}
</script>

<template>
  <component
    :is="list.ordered ? 'ol' : 'ul'"
    class="markdown-block__list"
    :start="listStart(list)"
  >
    <li
      v-for="(item, itemIndex) in list.items"
      :key="itemIndex"
      class="markdown-block__list-item"
      :class="{ 'markdown-block__list-item--task': item.taskChecked !== null }"
    >
      <div class="markdown-block__list-item-content">
        <input
          v-if="item.taskChecked !== null"
          class="markdown-block__task-checkbox"
          type="checkbox"
          :checked="item.taskChecked"
          disabled
          :aria-label="item.taskChecked ? '已完成' : '未完成'"
        >
        <span class="markdown-block__list-item-text">
          <MarkdownInline :tokens="item.inlines" />
        </span>
      </div>
      <MarkdownList
        v-for="(child, childIndex) in item.children"
        :key="childIndex"
        :list="child"
      />
    </li>
  </component>
</template>
