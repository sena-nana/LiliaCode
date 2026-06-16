<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowUp,
  Camera,
  CodeXml,
  Combine,
  FileQuestion,
  GitBranch,
  GitCommit,
  GitFork,
  Globe,
  ListChecks,
  Paperclip,
  SlidersHorizontal,
  ShieldCheck,
  Square,
} from "lucide-vue-next";
import type {
  ChatAttachment,
  ChatComposerState,
  ChatRuntimeCommand,
  LiliaReviewTarget,
  PermissionMode,
  ProviderRuntimeOptions,
} from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";
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
  sessionForkDisabled: boolean;
  providerSettingsDisabled: boolean;
  sendTitle: string;
  sendAriaLabel: string;
}>();

const emit = defineEmits<{
  pickAttachments: [];
  setPermission: [permission: PermissionMode];
  togglePlanMode: [];
  startLiliaReview: [target: LiliaReviewTarget];
  startLiliaFixSuggestion: [target: LiliaReviewTarget];
  startLiliaCompact: [];
  startSessionFork: [];
  applyLiliaProviderSettings: [
    runtimeCommand: Extract<ChatRuntimeCommand, { type: "runtime_settings" }>,
    runtimeOptions?: ProviderRuntimeOptions | null,
  ];
  openLiliaIab: [];
  submitLiliaIab: [];
  submitEntry: [];
  openImage: [attachment: ChatAttachment];
}>();

const reviewOpen = ref(false);
const reviewRoot = ref<HTMLElement | null>(null);
const fixSuggestionOpen = ref(false);
const fixSuggestionRoot = ref<HTMLElement | null>(null);
const providerSettingsOpen = ref(false);
const providerSettingsRoot = ref<HTMLElement | null>(null);

function closeReviewMenu() {
  reviewOpen.value = false;
}

function closeFixSuggestionMenu() {
  fixSuggestionOpen.value = false;
}

function closeProviderSettingsMenu() {
  providerSettingsOpen.value = false;
}

function toggleReviewMenu() {
  if (props.reviewDisabled || props.actionsBlocked) return;
  reviewOpen.value = !reviewOpen.value;
}

function toggleFixSuggestionMenu() {
  if (props.fixSuggestionDisabled || props.actionsBlocked) return;
  fixSuggestionOpen.value = !fixSuggestionOpen.value;
}

function toggleProviderSettingsMenu() {
  if (props.providerSettingsDisabled || props.actionsBlocked) return;
  providerSettingsOpen.value = !providerSettingsOpen.value;
}

function onDocPointer(e: PointerEvent) {
  if (reviewRoot.value && !reviewRoot.value.contains(e.target as Node)) closeReviewMenu();
  if (
    fixSuggestionRoot.value &&
    !fixSuggestionRoot.value.contains(e.target as Node)
  ) {
    closeFixSuggestionMenu();
  }
  if (
    providerSettingsRoot.value &&
    !providerSettingsRoot.value.contains(e.target as Node)
  ) {
    closeProviderSettingsMenu();
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
  if (e.key === "Escape" && providerSettingsOpen.value) {
    closeProviderSettingsMenu();
    e.stopPropagation();
  }
}

function startReview(target: LiliaReviewTarget) {
  closeReviewMenu();
  emit("startLiliaReview", target);
}

function startFixSuggestion(target: LiliaReviewTarget) {
  closeFixSuggestionMenu();
  emit("startLiliaFixSuggestion", target);
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

function runtimeOptionsForCurrentComposer(): ProviderRuntimeOptions {
  const options: ProviderRuntimeOptions = {
    common: {
      permission: props.state.permission,
    },
    provider: props.state.backend === "codex" ? { codex: {} } : { claude: {} },
  };
  const model = props.state.model.trim();
  if (model) options.common!.model = model;
  return options;
}

function diagnoseProviderSettings() {
  closeProviderSettingsMenu();
  emit(
    "applyLiliaProviderSettings",
    { type: "runtime_settings", action: "diagnose" },
    runtimeOptionsForCurrentComposer(),
  );
}

function updateProviderSettings() {
  closeProviderSettingsMenu();
  const model = window.prompt("Provider model", props.state.model)?.trim();
  if (model === undefined) return;
  const permission = window.prompt("权限：full / ask / readonly", props.state.permission)?.trim();
  if (permission === undefined) return;
  if (permission && !["full", "ask", "readonly"].includes(permission)) return;

  const runtimeOptions = runtimeOptionsForCurrentComposer();
  runtimeOptions.common ??= {};
  if (model) runtimeOptions.common.model = model;
  if (permission) runtimeOptions.common.permission = permission as PermissionMode;

  if (props.state.backend === "codex") {
    const reasoningEffort = window.prompt("Codex reasoning effort（留空不修改）", "")?.trim();
    if (reasoningEffort === undefined) return;
    runtimeOptions.provider = {
      codex: reasoningEffort ? { reasoningEffort } : {},
    };
  } else {
    const rawMaxTurns = window.prompt("Claude max turns（留空不修改）", "")?.trim();
    if (rawMaxTurns === undefined) return;
    let maxTurns: number | undefined;
    if (rawMaxTurns) {
      maxTurns = Number.parseInt(rawMaxTurns, 10);
      if (!Number.isFinite(maxTurns) || maxTurns <= 0) return;
    }
    runtimeOptions.provider = {
      claude: maxTurns ? { maxTurns } : {},
    };
  }

  emit(
    "applyLiliaProviderSettings",
    { type: "runtime_settings", action: "update" },
    runtimeOptions,
  );
}

function supportsBuiltinAgentActions(backend: ChatComposerState["backend"]) {
  return backend === "codex" || backend === "claude";
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
  [reviewOpen, fixSuggestionOpen, providerSettingsOpen],
  ([review, fixSuggestion, providerSettings]) => {
    syncDocumentListeners(review || fixSuggestion || providerSettings);
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
        <div ref="fixSuggestionRoot" class="chat-review-menu">
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
        <button
          v-if="supportsBuiltinAgentActions(state.backend)"
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-disabled': sessionForkDisabled }"
          :disabled="sessionForkDisabled || actionsBlocked"
          title="分叉当前会话"
          aria-label="分叉当前会话"
          @click="emit('startSessionFork')"
        >
          <GitFork :size="14" aria-hidden="true" />
        </button>
        <div
          v-if="supportsBuiltinAgentActions(state.backend)"
          ref="providerSettingsRoot"
          class="chat-review-menu"
        >
          <button
            type="button"
            class="chat-chip chat-chip--icon"
            :class="{ 'is-open': providerSettingsOpen, 'is-disabled': providerSettingsDisabled }"
            :disabled="providerSettingsDisabled || actionsBlocked"
            title="Provider runtime 设置"
            aria-label="Provider runtime 设置"
            :aria-haspopup="true"
            :aria-expanded="providerSettingsOpen"
            @click="toggleProviderSettingsMenu"
          >
            <SlidersHorizontal :size="14" aria-hidden="true" />
          </button>
          <div
            v-if="providerSettingsOpen"
            class="dd__menu dd__menu--top chat-review-menu__menu"
            role="menu"
          >
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="diagnoseProviderSettings"
            >
              <SlidersHorizontal :size="14" aria-hidden="true" />
              <span class="dd__item-label">诊断当前设置</span>
            </button>
            <button
              type="button"
              class="dd__item"
              role="menuitem"
              @click="updateProviderSettings"
            >
              <ShieldCheck :size="14" aria-hidden="true" />
              <span class="dd__item-label">更新 runtime 设置...</span>
            </button>
          </div>
        </div>
        <button
          v-if="supportsBuiltinAgentActions(state.backend)"
          type="button"
          class="chat-chip chat-chip--icon"
          :disabled="actionsBlocked"
          title="打开 Lilia IAB"
          aria-label="打开 Lilia IAB"
          @click="emit('openLiliaIab')"
        >
          <Globe :size="14" aria-hidden="true" />
        </button>
        <button
          v-if="supportsBuiltinAgentActions(state.backend)"
          type="button"
          class="chat-chip chat-chip--icon"
          :disabled="actionsBlocked"
          title="回送 IAB 截图"
          aria-label="回送 IAB 截图"
          @click="emit('submitLiliaIab')"
        >
          <Camera :size="14" aria-hidden="true" />
        </button>
        <button
          v-if="supportsBuiltinAgentActions(state.backend)"
          type="button"
          class="chat-chip chat-chip--icon"
          :class="{ 'is-disabled': compactDisabled }"
          :disabled="compactDisabled || actionsBlocked"
          title="压缩上下文"
          aria-label="压缩上下文"
          @click="emit('startLiliaCompact')"
        >
          <Combine :size="14" aria-hidden="true" />
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
