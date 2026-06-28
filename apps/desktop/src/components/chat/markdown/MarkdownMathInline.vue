<script setup lang="ts">
import "katex/dist/katex.min.css";
import { ref } from "vue";
import { useDeferredMathRender } from "./useDeferredMathRender";

const props = defineProps<{
  source: string;
}>();

const host = ref<HTMLElement | null>(null);

const { renderedHtml } = useDeferredMathRender({
  source: () => props.source,
  target: () => host.value,
  displayMode: false,
  perfName: "markdown.math-inline.render",
  visibilityPerfName: "markdown.math-inline.visible",
});
</script>

<template>
  <span ref="host">
    <span
      v-if="renderedHtml"
      class="markdown-block__math-inline"
      v-html="renderedHtml"
    />
    <template v-else>{{ source }}</template>
  </span>
</template>

