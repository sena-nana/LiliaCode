<script setup lang="ts">
import type { InlineToken } from "./markdownParser";

withDefaults(defineProps<{
  tokens: InlineToken[];
  renderImages?: boolean;
}>(), {
  renderImages: true,
});

function linkTarget(href: string | null): string | undefined {
  return href && /^https?:/i.test(href) ? "_blank" : undefined;
}
</script>

<template>
  <template v-for="(token, index) in tokens" :key="`${token.type}:${index}`">
    <code v-if="token.type === 'code'">{{ token.text }}</code>
    <span
      v-else-if="token.type === 'math'"
      class="markdown-block__math-inline"
      v-html="token.html"
    />
    <strong v-else-if="token.type === 'strong'">{{ token.text }}</strong>
    <em v-else-if="token.type === 'em'">{{ token.text }}</em>
    <del v-else-if="token.type === 'delete'">{{ token.text }}</del>
    <br v-else-if="token.type === 'break'">
    <img
      v-else-if="token.type === 'image' && token.href && renderImages"
      class="markdown-block__image"
      :src="token.href"
      :alt="token.text"
      loading="lazy"
    >
    <a
      v-else-if="token.type === 'link' && token.href"
      :href="token.href"
      :target="linkTarget(token.href)"
      rel="noreferrer"
    >{{ token.text }}</a>
    <template v-else>{{ token.text }}</template>
  </template>
</template>
