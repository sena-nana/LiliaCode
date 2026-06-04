<script setup lang="ts">
import type { Component } from "vue";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
} from "lucide-vue-next";
import type { ToolConsentRequest } from "../../services/chat";
import EditableCommandBlock from "./EditableCommandBlock.vue";

withDefaults(defineProps<{
  activeToolConsent: ToolConsentRequest;
  hasEditableCommand: boolean;
  isEditingToolCommand: boolean;
  rootClass?: string;
  showInlinePreview?: boolean;
  toolCommandDraft: string;
  toolDanger: boolean;
  toolExpanded: boolean;
  toolHeadline: string;
  toolIcon: Component;
  toolInlinePreview?: string | null;
  toolInputJson: string | null;
  toolSubtitle: string | null;
}>(), {
  rootClass: "composer-inline composer-inline--tool",
  showInlinePreview: false,
  toolInlinePreview: null,
});

const emit = defineEmits<{
  beginCommandEdit: [];
  updateToolCommandDraft: [draft: string];
  updateToolExpanded: [expanded: boolean];
}>();
</script>

<template>
  <section
    :class="[
      rootClass,
      {
        'composer-inline--danger': toolDanger,
        'is-expanded': toolExpanded,
        'is-editing-command': isEditingToolCommand,
      },
    ]"
    role="alert"
    aria-live="assertive"
  >
    <div class="composer-inline__tool-row">
      <span class="composer-inline__icon" aria-hidden="true">
        <AlertTriangle v-if="toolDanger" :size="14" />
        <component v-else :is="toolIcon" :size="14" />
      </span>

      <div class="composer-inline__tool-main">
        <div class="composer-inline__tool-head">
          <span class="composer-inline__tool-name">{{ activeToolConsent.toolName }}</span>
          <span class="composer-inline__headline">{{ toolHeadline }}</span>
        </div>
        <p
          v-if="showInlinePreview && toolInlinePreview && !hasEditableCommand"
          class="composer-inline__preview-line"
        >
          {{ toolInlinePreview }}
        </p>
        <p v-if="toolSubtitle" class="composer-inline__subtitle">{{ toolSubtitle }}</p>
      </div>

      <button
        v-if="toolInputJson && toolInputJson !== '{}'"
        type="button"
        class="composer-inline__toggle"
        :aria-expanded="toolExpanded"
        @click="emit('updateToolExpanded', !toolExpanded)"
      >
        <component
          :is="toolExpanded ? ChevronDown : ChevronRight"
          :size="12"
          aria-hidden="true"
        />
        {{ toolExpanded ? "收起" : "查看入参" }}
      </button>
    </div>

    <EditableCommandBlock
      v-if="hasEditableCommand"
      :model-value="toolCommandDraft"
      :editing="isEditingToolCommand"
      @update:model-value="emit('updateToolCommandDraft', $event)"
      @begin-edit="emit('beginCommandEdit')"
    />

    <pre v-if="toolExpanded" class="composer-inline__details">{{ toolInputJson }}</pre>
    <slot />
  </section>
</template>
