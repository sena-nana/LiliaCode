<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import {
  ArrowUp,
  Camera,
  CodeXml,
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
  ChatContextUsage,
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
  contextUsage?: ChatContextUsage | null;
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
const numberFormatter = new Intl.NumberFormat("zh-CN");

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

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

function contextUsageTone(usage: ChatContextUsage | null | undefined): string {
  const percent = usage?.usedPercent;
  if (typeof percent !== "number" || !Number.isFinite(percent)) return "chat-context-ring--empty";
  if (percent >= 100) return "chat-context-ring--error";
  if (percent >= 85) return "chat-context-ring--warn";
  return "chat-context-ring--normal";
}

function formatTokens(value: number | null | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "--";
  return numberFormatter.format(Math.max(0, Math.trunc(value)));
}

function formatContextUsageTime(value: number | null | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) return "未知时间";
  return new Date(value).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

const contextUsageProgressStyle = computed<Record<string, string>>(() => ({
  "--quota-progress": String(clampPercent(props.contextUsage?.usedPercent ?? 0)),
}));

function formatContextUsagePercent(usage: ChatContextUsage): string {
  return typeof usage.usedPercent === "number" && Number.isFinite(usage.usedPercent)
    ? `${clampPercent(usage.usedPercent).toFixed(1)}%`
    : "未知";
}

const contextUsageTitle = computed(() => {
  const usage = props.contextUsage;
  if (!usage) return "暂无上下文占用数据，点击压缩上下文";
  const parts = [
    `压缩上下文`,
    `已用 ${formatTokens(usage.usedTokens)} tokens`,
    usage.limitTokens ? `上限 ${formatTokens(usage.limitTokens)} tokens` : "上限未知",
    typeof usage.usedPercent === "number" && Number.isFinite(usage.usedPercent)
      ? `占用 ${clampPercent(usage.usedPercent).toFixed(1)}%`
      : "占用比例未知",
    `来源 ${usage.source || "runtime"}`,
    `更新 ${formatContextUsageTime(usage.updatedAt)}`,
  ];
  if (usage.unavailableReason) parts.push(usage.unavailableReason);
  return parts.join(" · ");
});

const contextUsageRows = computed(() => {
  const usage = props.contextUsage;
  if (!usage) {
    return [
      { label: "状态", value: "暂无上下文占用数据" },
      { label: "操作", value: "点击压缩上下文" },
    ];
  }
  const rows = [
    { label: "已用", value: `${formatTokens(usage.usedTokens)} tokens` },
    {
      label: "上限",
      value: usage.limitTokens ? `${formatTokens(usage.limitTokens)} tokens` : "未知",
    },
    { label: "占用", value: formatContextUsagePercent(usage) },
    { label: "来源", value: usage.source || "runtime" },
    { label: "更新", value: formatContextUsageTime(usage.updatedAt) },
  ];
  if (usage.unavailableReason) rows.push({ label: "说明", value: usage.unavailableReason });
  return rows;
});

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
      </div>

      <div class="chat-composer__entry-actions">
        <div
          v-if="supportsBuiltinAgentActions(state.backend)"
          class="chat-composer__context-wrap"
        >
          <button
            type="button"
            class="chat-composer__context-action"
            :class="[contextUsageTone(contextUsage), { 'is-disabled': compactDisabled }]"
            :style="contextUsageProgressStyle"
            :disabled="compactDisabled || actionsBlocked"
            :aria-label="contextUsageTitle"
            @click="emit('startLiliaCompact')"
          >
            <svg viewBox="0 0 16 16" focusable="false" aria-hidden="true">
              <circle class="chat-context-ring__track" cx="8" cy="8" r="6" pathLength="100" />
              <circle class="chat-context-ring__value" cx="8" cy="8" r="6" pathLength="100" />
            </svg>
          </button>
          <div
            class="chat-composer__context-card"
            role="tooltip"
          >
            <dl class="chat-composer__context-card-list">
              <template v-for="row in contextUsageRows" :key="row.label">
                <dt>{{ row.label }}</dt>
                <dd>{{ row.value }}</dd>
              </template>
            </dl>
          </div>
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
  </div>
</template>
