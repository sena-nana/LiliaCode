<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch, type Component } from "vue";
import {
  ArrowUp,
  ListChecks,
  Paperclip,
  Plus,
  MessageSquareQuote,
  Goal,
  ShieldCheck,
  Square,
  X,
} from "lucide-vue-next";
import type {
  ChatAttachment,
  ChatComposerState,
  ChatContextUsage,
  PermissionMode,
} from "@lilia/contracts";
import Dropdown from "../Dropdown.vue";
import { attachmentImageSrc } from "./imageViewer";
import {
  SB_MENU_POP_TRANSITION_MS,
} from "../../composables/menuMotion";
import { useAnchoredMenuMotion } from "../../composables/useAnchoredMenuMotion";

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
  referenceConversation: [];
  setPermission: [permission: PermissionMode];
  togglePlanMode: [];
  toggleGoalMode: [];
  startLiliaCompact: [];
  submitEntry: [];
  openImage: [attachment: ChatAttachment];
}>();

const numberFormatter = new Intl.NumberFormat("zh-CN");
const actionMenuOpen = ref(false);
const actionMenuPlacement = ref<"top" | "bottom">("top");
const {
  rootEl: actionMenuRoot,
  triggerEl: actionTriggerEl,
  menuEl: actionMenuEl,
  origin: actionMenuOrigin,
  captureAnchor: captureActionMenuAnchor,
  resolveInitialOrigin: resolveActionMenuInitialOrigin,
  updateOrigin: updateActionMenuOrigin,
} = useAnchoredMenuMotion(actionMenuPlacement);

type ModeChip = {
  key: "plan" | "goal";
  label: string;
  title: string;
  icon: Component;
  toggle: () => void;
};

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

const activeModeChips = computed<ModeChip[]>(() => {
  const chips: ModeChip[] = [];
  if (props.state.planMode) {
    chips.push({
      key: "plan",
      label: "计划",
      title: "关闭计划模式",
      icon: ListChecks,
      toggle: () => emit("togglePlanMode"),
    });
  }
  if (props.state.goalMode) {
    chips.push({
      key: "goal",
      label: "目标",
      title: "关闭目标模式",
      icon: Goal,
      toggle: () => emit("toggleGoalMode"),
    });
  }
  return chips;
});

function toggleActionMenu(event: MouseEvent) {
  captureActionMenuAnchor(event);
  actionMenuOpen.value = !actionMenuOpen.value;
}

function closeActionMenu() {
  actionMenuOpen.value = false;
}

function pickAction(action: "attachments" | "reference" | "plan" | "goal") {
  closeActionMenu();
  if (action === "attachments") emit("pickAttachments");
  else if (action === "reference") emit("referenceConversation");
  else if (action === "plan") emit("togglePlanMode");
  else emit("toggleGoalMode");
}

function onDocPointer(e: PointerEvent) {
  if (!actionMenuRoot.value) return;
  if (!actionMenuRoot.value.contains(e.target as Node)) closeActionMenu();
}

function onKey(e: KeyboardEvent) {
  if (e.key !== "Escape" || !actionMenuOpen.value) return;
  closeActionMenu();
  e.stopPropagation();
}

watch(actionMenuOpen, (open) => {
  if (open) {
    resolveActionMenuInitialOrigin();
    void updateActionMenuOrigin();
    document.addEventListener("pointerdown", onDocPointer, true);
    document.addEventListener("keydown", onKey);
  } else {
    document.removeEventListener("pointerdown", onDocPointer, true);
    document.removeEventListener("keydown", onKey);
  }
});

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
        <div ref="actionMenuRoot" class="chat-composer__action-menu">
          <button
            ref="actionTriggerEl"
            type="button"
            class="chat-composer__action-trigger"
            :class="{ 'is-open': actionMenuOpen }"
            title="更多输入操作"
            aria-label="更多输入操作"
            aria-haspopup="menu"
            :aria-expanded="actionMenuOpen"
            @click="toggleActionMenu"
          >
            <Plus :size="16" aria-hidden="true" />
          </button>
          <Transition name="sb-menu-pop" :duration="SB_MENU_POP_TRANSITION_MS">
            <div
              v-if="actionMenuOpen"
              ref="actionMenuEl"
              class="dd__menu dd__menu--top chat-composer__action-menu-popover"
              role="menu"
              :style="{
                '--sb-menu-origin-x': `${actionMenuOrigin.x}px`,
                '--sb-menu-origin-y': `${actionMenuOrigin.y}px`,
              }"
            >
              <button
                type="button"
                class="dd__item chat-composer__action-menu-item"
                role="menuitem"
                @click="pickAction('attachments')"
              >
                <span class="chat-composer__action-menu-icon">
                  <Paperclip :size="14" aria-hidden="true" />
                </span>
                <span class="dd__item-label">添加附件</span>
              </button>
              <button
                type="button"
                class="dd__item chat-composer__action-menu-item"
                role="menuitem"
                @click="pickAction('reference')"
              >
                <span class="chat-composer__action-menu-icon">
                  <MessageSquareQuote :size="14" aria-hidden="true" />
                </span>
                <span class="dd__item-label">引用其他对话</span>
              </button>
              <button
                type="button"
                class="dd__item chat-composer__action-menu-item chat-composer__action-menu-toggle"
                :class="{ 'is-active': state.planMode }"
                role="menuitemcheckbox"
                :aria-checked="state.planMode"
                @click="pickAction('plan')"
              >
                <span class="chat-composer__action-menu-icon">
                  <ListChecks :size="14" aria-hidden="true" />
                </span>
                <span class="dd__item-label">计划模式</span>
                <span class="chat-composer__action-switch" aria-hidden="true">
                  <span class="chat-composer__action-switch-thumb" />
                </span>
              </button>
              <button
                type="button"
                class="dd__item chat-composer__action-menu-item chat-composer__action-menu-toggle"
                :class="{ 'is-active': state.goalMode }"
                role="menuitemcheckbox"
                :aria-checked="state.goalMode"
                @click="pickAction('goal')"
              >
                <span class="chat-composer__action-menu-icon">
                  <Goal :size="14" aria-hidden="true" />
                </span>
                <span class="dd__item-label">目标模式</span>
                <span class="chat-composer__action-switch" aria-hidden="true">
                  <span class="chat-composer__action-switch-thumb" />
                </span>
              </button>
            </div>
          </Transition>
        </div>
        <Dropdown
          class="chat-composer__permission-dropdown"
          :class="{ 'is-full-access': state.permission === 'full' }"
          :model-value="state.permission"
          :options="permissionOptions"
          :icon="ShieldCheck"
          @update:model-value="emit('setPermission', $event)"
        />
        <button
          v-for="chip in activeModeChips"
          :key="chip.key"
          type="button"
          class="chat-composer__mode-chip"
          :title="chip.title"
          :aria-label="chip.title"
          @click="chip.toggle"
        >
          <span class="chat-composer__mode-chip-icon-slot" aria-hidden="true">
            <component
              :is="chip.icon"
              :size="12"
              class="chat-composer__mode-chip-icon"
            />
            <X :size="11" class="chat-composer__mode-chip-delete" />
          </span>
          <span class="chat-composer__mode-chip-label">{{ chip.label }}</span>
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
