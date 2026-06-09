<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowUp,
  Brain,
  CodeXml,
  Combine,
  FileQuestion,
  FileCog,
  TerminalSquare,
  GitBranch,
  GitCommit,
  GitFork,
  ListChecks,
  Paperclip,
  RotateCcw,
  ShieldCheck,
  SlidersHorizontal,
  Square,
} from "lucide-vue-next";
import type {
  ChatAttachment,
  ChatComposerState,
  CodexComposerSettings,
  CodexReviewTarget,
  PermissionMode,
} from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";
import CodexAdvancedSettingsPanel from "./CodexAdvancedSettingsPanel.vue";
import { attachmentImageSrc } from "./imageViewer";

const props = defineProps<{
  state: ChatComposerState;
  permissionOptions: Array<{ value: PermissionMode; label: string; hint: string }>;
  previewAttachments: ChatAttachment[];
  canInterrupt: boolean;
  canSubmitEntry: boolean;
  actionsBlocked: boolean;
  reviewDisabled: boolean;
  fixSuggestionDisabled: boolean;
  compactDisabled: boolean;
  backgroundTerminalsCleanDisabled: boolean;
  codexWorkflowDisabled: boolean;
  sendTitle: string;
  sendAriaLabel: string;
}>();

const emit = defineEmits<{
  pickAttachments: [];
  setPermission: [permission: PermissionMode];
  togglePlanMode: [];
  updateCodexSettings: [patch: CodexComposerSettings];
  startCodexReview: [target: CodexReviewTarget];
  startCodexFixSuggestion: [target: CodexReviewTarget];
  startCodexCompact: [];
  cleanCodexBackgroundTerminals: [];
  setCodexMemoryMode: [mode: "enabled" | "disabled"];
  resetCodexMemory: [];
  forkCodexThread: [];
  readCodexConfigDiagnostics: [];
  submitEntry: [];
  openImage: [attachment: ChatAttachment];
}>();

const reviewOpen = ref(false);
const reviewRoot = ref<HTMLElement | null>(null);
const fixSuggestionOpen = ref(false);
const fixSuggestionRoot = ref<HTMLElement | null>(null);
const codexWorkflowOpen = ref(false);
const codexWorkflowRoot = ref<HTMLElement | null>(null);
const codexSettingsOpen = ref(false);
const codexSettingsRoot = ref<HTMLElement | null>(null);

function closeReviewMenu() {
  reviewOpen.value = false;
}

function closeFixSuggestionMenu() {
  fixSuggestionOpen.value = false;
}

function closeCodexWorkflowMenu() {
  codexWorkflowOpen.value = false;
}

function toggleReviewMenu() {
  if (props.reviewDisabled || props.actionsBlocked) return;
  reviewOpen.value = !reviewOpen.value;
}

function toggleFixSuggestionMenu() {
  if (props.fixSuggestionDisabled || props.actionsBlocked) return;
  fixSuggestionOpen.value = !fixSuggestionOpen.value;
}

function toggleCodexWorkflowMenu() {
  if (props.codexWorkflowDisabled || props.actionsBlocked) return;
  codexWorkflowOpen.value = !codexWorkflowOpen.value;
}

function closeCodexSettings() {
  codexSettingsOpen.value = false;
}

function toggleCodexSettings() {
  if (props.state.backend !== "codex" || props.actionsBlocked) return;
  codexSettingsOpen.value = !codexSettingsOpen.value;
}

function onDocPointer(e: PointerEvent) {
  if (reviewRoot.value && !reviewRoot.value.contains(e.target as Node)) closeReviewMenu();
  if (
    fixSuggestionRoot.value &&
    !fixSuggestionRoot.value.contains(e.target as Node)
  ) {
    closeFixSuggestionMenu();
  }
  if (codexWorkflowRoot.value && !codexWorkflowRoot.value.contains(e.target as Node)) {
    closeCodexWorkflowMenu();
  }
  if (codexSettingsRoot.value && !codexSettingsRoot.value.contains(e.target as Node)) {
    closeCodexSettings();
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && reviewOpen.value) {
    closeReviewMenu();
    e.stopPropagation();
  }
  if (e.key === "Escape" && fixSuggestionOpen.value) {
    closeFixSuggestionMenu();
    e.stopPropagation();
  }
  if (e.key === "Escape" && codexWorkflowOpen.value) {
    closeCodexWorkflowMenu();
    e.stopPropagation();
  }
  if (e.key === "Escape" && codexSettingsOpen.value) {
    closeCodexSettings();
    e.stopPropagation();
  }
}

function startReview(target: CodexReviewTarget) {
  closeReviewMenu();
  emit("startCodexReview", target);
}

function startFixSuggestion(target: CodexReviewTarget) {
  closeFixSuggestionMenu();
  emit("startCodexFixSuggestion", target);
}

function startBranchReview() {
  const branch = window.prompt("对比分支")?.trim();
  if (branch) startReview({ type: "baseBranch", branch });
  else closeReviewMenu();
}

function startCommitReview() {
  const sha = window.prompt("指定提交")?.trim();
  if (sha) startReview({ type: "commit", sha });
  else closeReviewMenu();
}

function startBranchFixSuggestion() {
  const branch = window.prompt("对比分支")?.trim();
  if (branch) startFixSuggestion({ type: "baseBranch", branch });
  else closeFixSuggestionMenu();
}

function startCommitFixSuggestion() {
  const sha = window.prompt("指定提交")?.trim();
  if (sha) startFixSuggestion({ type: "commit", sha });
  else closeFixSuggestionMenu();
}

function setMemoryMode(mode: "enabled" | "disabled") {
  closeCodexWorkflowMenu();
  emit("setCodexMemoryMode", mode);
}

function resetMemory() {
  closeCodexWorkflowMenu();
  if (!window.confirm("重置 Codex memory？")) return;
  emit("resetCodexMemory");
}

function forkThread() {
  closeCodexWorkflowMenu();
  emit("forkCodexThread");
}

function readConfigDiagnostics() {
  closeCodexWorkflowMenu();
  emit("readCodexConfigDiagnostics");
}

function syncDocumentListeners(open: boolean) {
  if (open) {
    document.addEventListener("pointerdown", onDocPointer, true);
    document.addEventListener("keydown", onKey);
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onKey);
  }
}

watch(
  [reviewOpen, fixSuggestionOpen, codexWorkflowOpen, codexSettingsOpen],
  ([review, fixSuggestion, workflow, settings]) => {
    syncDocumentListeners(review || fixSuggestion || workflow || settings);
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointer, true);
  document.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div class="chat-composer__toolbar">
    <div
      v-if="previewAttachments.length"
      class="chat-composer__attachments"
      aria-label="图片预览"
    >
      <button
        v-for="attachment in previewAttachments"
        :key="attachment.id"
        type="button"
        class="chat-attachment-chip chat-attachment-chip--image-preview"
        :title="attachment.path"
        :aria-label="`查看图片 ${attachment.name}`"
        @click="emit('openImage', attachment)"
      >
        <img
          class="chat-attachment-chip__thumb"
          :src="attachmentImageSrc(attachment) ?? undefined"
          alt=""
        />
      </button>
    </div>

    <div class="chat-composer__row">
      <div class="chat-composer__group">
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          title="添加附件"
          aria-label="添加附件"
          @click="emit('pickAttachments')"
        >
          <Paperclip :size="14" aria-hidden="true" />
        </button>
        <Dropdown
          :model-value="state.permission"
          :options="permissionOptions"
          :icon="ShieldCheck"
          @update:model-value="emit('setPermission', $event)"
        />
        <button
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-open': state.planMode }"
          :title="state.planMode ? '本轮先制定计划' : '直接执行'"
          :aria-label="state.planMode ? '关闭计划模式' : '开启计划模式'"
          :aria-pressed="state.planMode"
          @click="emit('togglePlanMode')"
        >
          <ListChecks :size="14" aria-hidden="true" />
        </button>
        <div ref="reviewRoot" class="chat-review-menu">
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': reviewOpen, 'is-disabled': reviewDisabled }"
            :disabled="reviewDisabled || actionsBlocked"
            title="代码审查"
            aria-label="代码审查"
            :aria-haspopup="true"
            :aria-expanded="reviewOpen"
            @click="toggleReviewMenu"
          >
            <CodeXml :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="reviewOpen"
            class="dd__menu dd__menu--top chat-review-menu__menu"
            role="menu"
          >
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startReview({ type: 'uncommittedChanges' })"
            >
              <GitBranch :size="14" aria-hidden="true" />
              <span class="dd__item-label">未提交改动</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startBranchReview"
            >
              <GitBranch :size="14" aria-hidden="true" />
              <span class="dd__item-label">对比分支...</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startCommitReview"
            >
              <GitCommit :size="14" aria-hidden="true" />
              <span class="dd__item-label">指定提交...</span>
            </button>
          </div>
        </div>
        <div v-if="state.backend === 'codex'" ref="fixSuggestionRoot" class="chat-review-menu">
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': fixSuggestionOpen, 'is-disabled': fixSuggestionDisabled }"
            :disabled="fixSuggestionDisabled || actionsBlocked"
            title="修复建议"
            aria-label="修复建议"
            :aria-haspopup="true"
            :aria-expanded="fixSuggestionOpen"
            @click="toggleFixSuggestionMenu"
          >
            <FileQuestion :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="fixSuggestionOpen"
            class="dd__menu dd__menu--top chat-review-menu__menu"
            role="menu"
          >
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startFixSuggestion({ type: 'uncommittedChanges' })"
            >
              <GitBranch :size="14" aria-hidden="true" />
              <span class="dd__item-label">未提交改动</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startBranchFixSuggestion"
            >
              <GitBranch :size="14" aria-hidden="true" />
              <span class="dd__item-label">对比分支...</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="startCommitFixSuggestion"
            >
              <GitCommit :size="14" aria-hidden="true" />
              <span class="dd__item-label">指定提交...</span>
            </button>
          </div>
        </div>
        <div v-if="state.backend === 'codex'" ref="codexSettingsRoot" class="chat-review-menu">
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': codexSettingsOpen }"
            :disabled="actionsBlocked"
            title="Codex 高级字段"
            aria-label="Codex 高级字段"
            :aria-haspopup="true"
            :aria-expanded="codexSettingsOpen"
            @click="toggleCodexSettings"
          >
            <SlidersHorizontal :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="codexSettingsOpen"
            class="dd__menu dd__menu--top chat-review-menu__menu chat-codex-settings-menu"
            role="menu"
          >
            <CodexAdvancedSettingsPanel
              :value="state.codexSettings"
              :disabled="actionsBlocked"
              @update="emit('updateCodexSettings', $event)"
            />
          </div>
        </div>
        <div v-if="state.backend === 'codex'" ref="codexWorkflowRoot" class="chat-review-menu">
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': codexWorkflowOpen, 'is-disabled': codexWorkflowDisabled }"
            :disabled="codexWorkflowDisabled || actionsBlocked"
            title="Codex 原生接口"
            aria-label="Codex 原生接口"
            :aria-haspopup="true"
            :aria-expanded="codexWorkflowOpen"
            @click="toggleCodexWorkflowMenu"
          >
            <Brain :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="codexWorkflowOpen"
            class="dd__menu dd__menu--top chat-review-menu__menu"
            role="menu"
          >
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="setMemoryMode('enabled')"
            >
              <Brain :size="14" aria-hidden="true" />
              <span class="dd__item-label">启用 Memory</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="setMemoryMode('disabled')"
            >
              <Brain :size="14" aria-hidden="true" />
              <span class="dd__item-label">关闭 Memory</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="resetMemory"
            >
              <RotateCcw :size="14" aria-hidden="true" />
              <span class="dd__item-label">重置 Memory...</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="forkThread"
            >
              <GitFork :size="14" aria-hidden="true" />
              <span class="dd__item-label">Fork 当前 Thread</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="readConfigDiagnostics"
            >
              <FileCog :size="14" aria-hidden="true" />
              <span class="dd__item-label">读取配置诊断</span>
            </button>
          </div>
        </div>
        <button
          v-if="state.backend === 'codex'"
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-disabled': compactDisabled }"
          :disabled="compactDisabled || actionsBlocked"
          title="压缩 Codex 上下文"
          aria-label="压缩 Codex 上下文"
          @click="emit('startCodexCompact')"
        >
          <Combine :size="14" aria-hidden="true" />
        </button>
        <button
          v-if="state.backend === 'codex'"
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-disabled': backgroundTerminalsCleanDisabled }"
          :disabled="backgroundTerminalsCleanDisabled || actionsBlocked"
          title="清理 Codex 后台终端"
          aria-label="清理 Codex 后台终端"
          @click="emit('cleanCodexBackgroundTerminals')"
        >
          <TerminalSquare :size="14" aria-hidden="true" />
        </button>
      </div>

      <button
        type="button"
        class="chat-composer__send"
        :class="{ 'chat-composer__send--interrupt': canInterrupt }"
        :disabled="actionsBlocked || !canSubmitEntry"
        :title="sendTitle"
        :aria-label="sendAriaLabel"
        @click="emit('submitEntry')"
      >
        <component :is="canInterrupt ? Square : ArrowUp" :size="16" aria-hidden="true" />
      </button>
    </div>
  </div>
</template>
