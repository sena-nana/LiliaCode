<script setup lang="ts">
import { computed } from "vue";
import type { SuggestionItem } from "@lilia/contracts";

const props = defineProps<{
  suggestions?: SuggestionItem[];
  suggestionsStatus?: "idle" | "loading" | "empty" | "error";
  suggestionsLoadingText?: string;
  suggestionsVisible?: boolean;
}>();

const emit = defineEmits<{
  select: [suggestion: SuggestionItem];
  "refresh-suggestions": [];
}>();

const suggestionRows = computed(() => props.suggestions ?? []);
const suggestionStatus = computed(() => props.suggestionsStatus ?? "idle");
const suggestionsLoadingText = computed(() =>
  props.suggestionsLoadingText?.trim() || "正在寻找灵感",
);
const showSuggestions = computed(() =>
  props.suggestionsVisible === true &&
  (suggestionRows.value.length > 0 || suggestionStatus.value !== "idle"),
);
const suggestionStatusText = computed(() => {
  if (suggestionRows.value.length > 0) return "";
  switch (suggestionStatus.value) {
    case "loading":
      return suggestionsLoadingText.value;
    case "error":
      return "建议暂时不可用";
    default:
      return "";
  }
});

type SuggestionGitHubActivity = SuggestionItem["githubActivities"][number];

function githubActivityAnchor(activity: SuggestionGitHubActivity): string {
  const title = activity.title.trim();
  if (activity.kind === "pull_request") {
    const number = title.match(/#(\d+)/)?.[1];
    return number ? `PR #${number}` : "PR";
  }
  if (activity.kind === "issue") {
    const number = title.match(/#(\d+)/)?.[1];
    return number ? `Issue #${number}` : "Issue";
  }
  if (activity.kind === "push") {
    const branch = title.match(/^Push\s+([^:]+):/i)?.[1]?.trim();
    return branch ? `Push ${branch}` : "Push";
  }
  return title || activity.kind || "GitHub";
}

function suggestionGitHubSourceLabel(suggestion: SuggestionItem): string {
  const githubActivities = suggestion.githubActivities ?? [];
  const [activity] = githubActivities;
  if (!activity) return "";
  const source = [
    activity.repoFullName.trim(),
    githubActivityAnchor(activity),
  ].filter(Boolean).join(" · ");
  const extraCount = githubActivities.length - 1;
  return extraCount > 0 ? `${source} +${extraCount}` : source;
}

function suggestionSourceLabel(suggestion: SuggestionItem): string {
  const githubLabel = suggestionGitHubSourceLabel(suggestion);
  if (githubLabel) return githubLabel;
  const localGitContexts = suggestion.localGitContexts ?? [];
  const [context] = localGitContexts;
  if (!context) return "";
  const branch = context.branch.trim();
  const source = branch ? `本地 Git · ${branch}` : "本地 Git";
  const extraCount = localGitContexts.length - 1;
  return extraCount > 0 ? `${source} +${extraCount}` : source;
}

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
          @mousedown.prevent
          @click="emit('refresh-suggestions')"
        >
          重试
        </button>
      </div>
    </div>
  </div>
</template>
