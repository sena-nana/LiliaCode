<script setup lang="ts">
import { computed } from "vue";
import {
  ArrowUp,
  ListChecks,
  Paperclip,
  ShieldCheck,
  Square,
} from "lucide-vue-next";
import type {
  ChatAttachment,
  ChatComposerState,
  ChatContextUsage,
  PermissionMode,
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
  compactDisabled: boolean;
  contextUsage?: ChatContextUsage | null;
  sendTitle: string;
  sendAriaLabel: string;
}>();

const emit = defineEmits<{
  pickAttachments: [];
  setPermission: [permission: PermissionMode];
  togglePlanMode: [];
  startLiliaCompact: [];
  submitEntry: [];
  openImage: [attachment: ChatAttachment];
}>();

const numberFormatter = new Intl.NumberFormat("zh-CN");

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
          class="chat-chip"
          :class="{ 'is-open': state.planMode }"
          :title="state.planMode ? '计划模式已开启' : '开启计划模式'"
          :aria-label="state.planMode ? '关闭计划模式' : '开启计划模式'"
          :aria-pressed="state.planMode"
          @click="emit('togglePlanMode')"
        >
          <ListChecks :size="14" aria-hidden="true" />
          <span class="chat-chip__label">计划模式</span>
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
