<script setup lang="ts">
import type { Component } from "vue";
import {
  AlertTriangle,
  ArrowLeft,
  Check,
  ChevronDown,
  ChevronRight,
  CircleHelp,
  X,
} from "lucide-vue-next";
import type { AskUserOption, AskUserQuestion } from "@lilia/contracts";
import type { PendingAsk } from "../../composables/useAskUser";
import type { ToolConsentRequest } from "../../services/chat";
import EditableCommandBlock from "./EditableCommandBlock.vue";

type AskOptionView = AskUserOption & { id: string };

defineProps<{
  activeAsk: PendingAsk | null;
  askQuestion: AskUserQuestion | null | undefined;
  askTitle: string;
  askIndex: number;
  askTotal: number;
  askDismissable: boolean;
  askIsPlanApproval: boolean;
  askOptionsWithId: AskOptionView[];
  askHasPreview: boolean;
  askFocusedOption: AskOptionView | null | undefined;
  activeAskOptionId: string | null | undefined;
  singlePick: string | null | undefined;
  multiPicks: Set<string>;
  canGoPrev: boolean;
  activeToolConsent: ToolConsentRequest | null;
  toolDanger: boolean;
  toolIcon: Component;
  toolHeadline: string;
  toolInlinePreview: string | null;
  toolInputJson: string | null;
  toolSubtitle: string | null;
  toolExpanded: boolean;
  isEditingToolCommand: boolean;
  hasEditableCommand: boolean;
  toolCommandDraft: string;
}>();

const emit = defineEmits<{
  keydown: [event: KeyboardEvent];
  cancelAsk: [];
  highlightOption: [id: string];
  clearOptionHighlight: [id: string];
  focusOption: [id: string];
  selectSingleOption: [id: string];
  toggleMulti: [id: string];
  skipAsk: [];
  backAsk: [];
  confirmAskNo: [];
  submitAsk: [];
  updateToolExpanded: [expanded: boolean];
  updateToolCommandDraft: [draft: string];
  beginCommandEdit: [];
}>();
</script>

<template>
  <div class="chat-composer__pending-panel">
    <div class="chat-composer__pending-panel-inner">
      <section
        v-if="activeAsk && askQuestion"
        class="composer-inline composer-inline--ask"
        :class="{
          'composer-inline--danger': askQuestion.danger,
          'composer-inline--plan': askIsPlanApproval,
        }"
        role="region"
        aria-live="assertive"
        :aria-label="askTitle"
        tabindex="-1"
        @keydown="emit('keydown', $event)"
      >
        <header class="composer-inline__header">
          <span class="composer-inline__icon" aria-hidden="true">
            <AlertTriangle v-if="askQuestion.danger" :size="14" />
            <CircleHelp v-else :size="14" />
          </span>
          <span class="composer-inline__title">{{ askTitle }}</span>
          <span v-if="activeAsk.spec.source" class="composer-inline__source">
            {{ activeAsk.spec.source }}
          </span>
          <span v-if="askTotal > 1" class="composer-inline__progress" aria-live="polite">
            {{ askIndex + 1 }} / {{ askTotal }}
          </span>
          <button
            v-if="askDismissable"
            type="button"
            class="composer-inline__close"
            aria-label="关闭"
            @click="emit('cancelAsk')"
          >
            <X :size="14" />
          </button>
        </header>

        <div v-if="!askIsPlanApproval" class="composer-inline__body">
          <div class="composer-inline__question">
            <span
              v-if="askQuestion.header"
              class="composer-inline__chip"
            >{{ askQuestion.header }}</span>
            <p class="composer-inline__qtext">{{ askQuestion.question }}</p>
          </div>

          <div
            v-if="askQuestion.mode !== 'confirm'"
            class="composer-inline__main"
            :class="{ 'composer-inline__main--with-preview': askHasPreview }"
          >
            <ul
              class="composer-inline__options"
              :role="askQuestion.mode === 'single' ? 'radiogroup' : 'group'"
            >
              <li
                v-for="opt in askOptionsWithId"
                :key="opt.id"
                class="composer-inline__option"
                :class="{
                  'is-active': activeAskOptionId === opt.id,
                  'is-picked': askQuestion.mode === 'single'
                    ? singlePick === opt.id
                    : multiPicks.has(opt.id),
                  'is-recommended': opt.recommended,
                  'is-danger': opt.danger,
                }"
              >
                <button
                  type="button"
                  class="composer-inline__option-btn"
                  :role="askQuestion.mode === 'single' ? 'radio' : 'checkbox'"
                  :aria-checked="askQuestion.mode === 'single'
                    ? singlePick === opt.id
                    : multiPicks.has(opt.id)"
                  @mouseenter="emit('highlightOption', opt.id)"
                  @mouseleave="emit('clearOptionHighlight', opt.id)"
                  @focus="emit('focusOption', opt.id)"
                  @click="askQuestion.mode === 'single'
                    ? emit('selectSingleOption', opt.id)
                    : emit('toggleMulti', opt.id)"
                >
                  <span class="composer-inline__option-indicator" aria-hidden="true">
                    <Check
                      v-if="askQuestion.mode === 'multi' && multiPicks.has(opt.id)"
                      :size="12"
                    />
                  </span>
                  <span class="composer-inline__option-main">
                    <span class="composer-inline__option-label">
                      {{ opt.label }}
                      <span v-if="opt.recommended" class="composer-inline__badge">推荐</span>
                    </span>
                    <span
                      v-if="opt.description"
                      class="composer-inline__option-desc"
                    >{{ opt.description }}</span>
                  </span>
                </button>
              </li>
            </ul>

            <aside
              v-if="askHasPreview"
              class="composer-inline__preview"
              aria-label="选项预览"
            >
              <pre v-if="askFocusedOption?.preview" class="composer-inline__preview-pre">{{ askFocusedOption.preview }}</pre>
              <p v-else class="composer-inline__preview-empty">
                把鼠标移到选项上 / 用方向键聚焦，这里会显示对比预览。
              </p>
            </aside>
          </div>
        </div>

        <footer
          v-if="askQuestion.mode === 'confirm' && !askIsPlanApproval"
          class="composer-inline__actions"
        >
          <button
            v-if="askQuestion.skippable !== false && askTotal > 1"
            type="button"
            class="ghost composer-inline__skip composer-inline__btn"
            @click="emit('skipAsk')"
          >
            跳过
          </button>
          <span class="composer-inline__spacer" />
          <button
            v-if="canGoPrev"
            type="button"
            class="ghost composer-inline__btn"
            @click="emit('backAsk')"
          >
            <ArrowLeft :size="13" aria-hidden="true" />
            上一题
          </button>

          <button type="button" class="ghost composer-inline__btn" @click="emit('confirmAskNo')">
            {{ askQuestion.cancelLabel ?? "不要" }}
          </button>
          <button
            type="button"
            class="composer-inline__btn"
            :class="askQuestion.danger ? 'ghost danger' : 'primary'"
            @click="emit('submitAsk')"
          >
            {{ askQuestion.confirmLabel ?? "好的" }}
          </button>
        </footer>
      </section>

      <section
        v-else-if="activeToolConsent"
        class="composer-inline composer-inline--tool"
        :class="{
          'composer-inline--danger': toolDanger,
          'is-expanded': toolExpanded,
          'is-editing-command': isEditingToolCommand,
        }"
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
              v-if="toolInlinePreview && !hasEditableCommand"
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
      </section>
    </div>
  </div>
</template>
