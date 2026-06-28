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
  displayMode: true,
  perfName: "markdown.math-block.render",
  visibilityPerfName: "markdown.math-block.visible",
});
</script>

<template>
  <div ref="host">
    <div
      v-if="renderedHtml"
      class="markdown-block__math-block"
      v-html="renderedHtml"
    />
    <pre v-else class="markdown-block__code"><code>{{ source }}</code></pre>
  </div>
</template>

