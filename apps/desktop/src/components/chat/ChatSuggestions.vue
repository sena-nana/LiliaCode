<script setup lang="ts">
import { computed } from "vue";
import {
  suggestionSourceLabel,
  suggestionStatusDisplayText,
  type SuggestionItem,
  type SuggestionStatus,
} from "@lilia/contracts";

const props = defineProps<{
  suggestions?: SuggestionItem[];
  suggestionsStatus?: SuggestionStatus;
  suggestionsLoadingText?: string;
  suggestionsVisible?: boolean;
}>();

const emit = defineEmits<{
  select: [suggestion: SuggestionItem];
  "refresh-suggestions": [];
}>();

const suggestionRows = computed(() => props.suggestions ?? []);
const suggestionStatus = computed(() => props.suggestionsStatus ?? "idle");
const showSuggestions = computed(() =>
  props.suggestionsVisible === true &&
  (suggestionRows.value.length > 0 || suggestionStatus.value !== "idle"),
);
const suggestionStatusText = computed(() =>
  suggestionStatusDisplayText({
    hasSuggestions: suggestionRows.value.length > 0,
    status: suggestionStatus.value,
    loadingText: props.suggestionsLoadingText,
  }),
);

const suggestionViewRows = computed(() =>
  suggestionRows.value.map((suggestion) => {
    const sourceLabel = suggestionSourceLabel(suggestion);
    return {
      suggestion,
      sourceLabel,
      title: sourceLabel ? `${suggestion.reason}\n${sourceLabel}` : suggestion.reason,
    };
  }),
);
</script>

<template>
  <div
    class="chat-suggestions"
    :class="{ 'is-hidden': !showSuggestions }"
    :aria-label="showSuggestions ? '新对话建议' : undefined"
    :aria-hidden="showSuggestions ? undefined : 'true'"
  >
    <div v-if="showSuggestions" class="chat-suggestions__items">
      <template v-if="suggestionRows.length > 0">
        <button
          v-for="row in suggestionViewRows"
          :key="row.suggestion.id"
          type="button"
          class="chat-suggestion"
          :class="{ 'chat-suggestion--with-source': row.sourceLabel }"
          :data-agent-id="`chat.suggestions.item.${row.suggestion.id}`"
          :title="row.title"
          @mousedown.prevent
          @click="emit('select', row.suggestion)"
        >
          <span class="chat-suggestion__summary">{{ row.suggestion.summary }}</span>
          <span
            v-if="row.sourceLabel"
            class="chat-suggestion__source"
          >
            {{ row.sourceLabel }}
          </span>
        </button>
      </template>
      <div
        v-else-if="suggestionStatus === 'loading'"
        class="chat-suggestions__status"
        role="status"
      >
        <span class="chat-suggestions__spinner" aria-hidden="true"></span>
        <span>{{ suggestionStatusText }}</span>
      </div>
      <button
        v-else-if="suggestionStatus === 'empty'"
        type="button"
        class="chat-suggestions__inspire"
        data-agent-id="chat.suggestions.refresh-empty"
        @mousedown.prevent
        @click="emit('refresh-suggestions')"
      >
        来点灵感？
      </button>
      <div
        v-else-if="suggestionStatus === 'error'"
        class="chat-suggestions__status chat-suggestions__status--error"
        role="status"
      >
        <span>{{ suggestionStatusText }}</span>
        <button
          type="button"
          class="chat-suggestions__retry"
          data-agent-id="chat.suggestions.retry"
          @mousedown.prevent
          @click="emit('refresh-suggestions')"
        >
          重试
        </button>
      </div>
    </div>
  </div>
</template>

