<script setup lang="ts">
import CodeXml from "@lucide/vue/dist/esm/icons/code-xml.mjs";
import FileQuestion from "@lucide/vue/dist/esm/icons/file-question-mark.mjs";
import GitBranch from "@lucide/vue/dist/esm/icons/git-branch.mjs";
import GitCommit from "@lucide/vue/dist/esm/icons/git-commit-horizontal.mjs";
import TerminalSquare from "@lucide/vue/dist/esm/icons/square-terminal.mjs";
import { chatSlashCommandSourceLabel, chatWorkflowSlashKindLabel } from "@lilia/contracts";
import type {
  ComposerSlashCommandItem,
  ComposerSlashTargetItem,
  ComposerWorkflowSlashKind,
} from "./useComposerSlashCommands";

defineProps<{
  results: ComposerSlashCommandItem[];
  targetItems: ComposerSlashTargetItem[];
  activeWorkflowKind: ComposerWorkflowSlashKind | null;
  activeIndex: number;
  loading: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  activate: [index: number];
  select: [result: ComposerSlashCommandItem];
  selectTarget: [target: ComposerSlashTargetItem];
}>();

function sourceLabel(result: ComposerSlashCommandItem): string {
  if (result.kind === "workflow") return "工作流";
  return chatSlashCommandSourceLabel(result.command.source);
}

function targetIcon(target: ComposerSlashTargetItem) {
  return target.id === "commit" ? GitCommit : GitBranch;
}

function commandIcon(result: ComposerSlashCommandItem) {
  if (result.workflowKind === "review") return CodeXml;
  if (result.workflowKind === "fix_suggestion") return FileQuestion;
  return TerminalSquare;
}
</script>

<template>
  <div
    class="chat-composer__context-panel chat-composer__slash-panel"
    role="listbox"
    aria-label="斜杠命令"
  >
    <div v-if="activeWorkflowKind" class="chat-composer__context-list">
      <button
        v-for="(target, index) in targetItems"
        :key="target.id"
        type="button"
        class="chat-composer__context-item chat-composer__slash-item"
        :class="{ 'is-active': index === activeIndex }"
        :data-agent-id="`chat.composer.slash-target.${target.id}`"
        role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent
        @mouseenter="emit('activate', index)"
        @click="emit('selectTarget', target)"
      >
        <span class="chat-composer__context-icon" aria-hidden="true">
          <component :is="targetIcon(target)" :size="14" />
        </span>
        <span class="chat-composer__context-main">
          <span class="chat-composer__context-name">{{ target.label }}</span>
          <span class="chat-composer__context-path">{{ target.hint }}</span>
        </span>
        <span class="chat-composer__context-meta">
          {{ chatWorkflowSlashKindLabel(activeWorkflowKind) }}
        </span>
      </button>
    </div>
    <p v-else-if="loading && !results.length" class="chat-composer__context-note">
      正在搜索命令…
    </p>
    <p v-else-if="error && !results.length" class="chat-composer__context-note is-error">
      {{ error }}
    </p>
    <div v-else-if="results.length" class="chat-composer__context-list">
      <button
        v-for="(result, index) in results"
        :key="result.command.id"
        type="button"
        class="chat-composer__context-item chat-composer__slash-item"
        :class="{ 'is-active': index === activeIndex }"
        :data-agent-id="`chat.composer.slash-command.${result.command.id}`"
        role="option"
        :aria-selected="index === activeIndex"
        @mousedown.prevent
        @mouseenter="emit('activate', index)"
        @click="emit('select', result)"
      >
        <span class="chat-composer__context-icon" aria-hidden="true">
          <component :is="commandIcon(result)" :size="14" />
        </span>
        <span class="chat-composer__context-main">
          <span class="chat-composer__context-name">/{{ result.command.name }}</span>
          <span class="chat-composer__context-path">{{ result.command.title }}</span>
        </span>
        <span class="chat-composer__context-meta">
          {{ sourceLabel(result) }}
        </span>
      </button>
    </div>
    <p v-else class="chat-composer__context-note">
      没有匹配的命令
    </p>
  </div>
</template>

