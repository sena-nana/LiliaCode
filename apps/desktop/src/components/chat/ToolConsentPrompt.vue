<script setup lang="ts">
/**
 * 工具调用授权 inline 卡片。挂在 ChatComposer 上方、贴着 .chat 网格的最后一行。
 *
 * 视觉与 ChatComposer / TodoFloat 共用一套几何（bg-elev + 14px 圆角 + 软阴影），
 * 不再用 modal 弹窗——授权交互是日常高频动作，弹窗会打断聊天节奏。
 *
 * 设计点：
 * - 视觉与 composer / todo-float 同款：bg-elev + 14px 圆角 + 轻阴影，不加色块/光晕。
 * - 只有真正具备破坏性的调用走 .tool-consent--danger：AlertTriangle 图标染红 +
 *   主按钮换 ghost danger，仅此两处信号；不再加红条、染边框、染图标背景。
 *   目前唯一会被升级成 danger 的是命中 DANGEROUS_BASH_RE 的 Bash 调用——
 *   如果每次同意都是红色，红色就失去预警意义。
 * - 默认折叠成一行；右下"查看入参"展开 code block 看全文。
 * - 一旦提交决策（或被 service 主动撤掉），父级 v-if=false → Transition 退场。
 */
import { computed, ref } from "vue";
import {
  AlertTriangle,
  Bot,
  ChevronDown,
  ChevronRight,
  FilePen,
  Globe,
  Search,
  Terminal,
  Wrench,
} from "lucide-vue-next";
import type { Component } from "vue";
import {
  respondConsent,
  useToolConsentForTask,
} from "../../composables/useToolConsentBridge";

const props = defineProps<{ taskId: string }>();

const current = useToolConsentForTask(props.taskId);
const expanded = ref(false);
/** 提交中：禁用按钮，避免连点。 */
const submitting = ref<"allow" | "deny" | null>(null);

const DANGEROUS_BASH_RE =
  /\b(rm\s+-[a-z]*r|rmdir\s+\/s|sudo\b|doas\b|chmod\s+-R|chown\s+-R|mkfs\b|dd\s+if=|fdisk\b|format\s+[a-z]:|del\s+\/[fsq]|rd\s+\/s|kill(all)?\s+-9|pkill\b|shutdown\b|reboot\b|halt\b|drop\s+(database|table|schema)|truncate\s+table|:\(\)\{\s*:\|:&\s*\};:)/i;

const TOOL_ICON_MAP: Record<string, Component> = {
  Bash: Terminal,
  Write: FilePen,
  Edit: FilePen,
  MultiEdit: FilePen,
  NotebookEdit: FilePen,
  WebFetch: Globe,
  WebSearch: Search,
  Grep: Search,
  Glob: Search,
  Read: FilePen,
  Agent: Bot,
  Task: Bot,
};

const danger = computed(() => {
  const c = current.value;
  if (!c) return false;
  // 只在 Bash 命令文本命中破坏性模式时才升级成 danger。
  // 写文件/抓网页本来就是 coding agent 的常规活，不应触发红色预警。
  if (c.toolName !== "Bash") return false;
  const cmd = (c.input as Record<string, unknown> | null | undefined)?.command;
  return typeof cmd === "string" && DANGEROUS_BASH_RE.test(cmd);
});

const toolIcon = computed<Component>(() => {
  const name = current.value?.toolName ?? "";
  return TOOL_ICON_MAP[name] ?? Wrench;
});

const headline = computed(() => {
  const c = current.value;
  if (!c) return "";
  const tool = c.displayName?.trim() || c.toolName || "工具";
  if (c.title?.trim()) return c.title.trim();
  return danger.value ? `想执行 ${tool}` : `想使用 ${tool}`;
});

/** 输入预览：尽量挑一个最具代表性的字段做一行摘要；找不到回退到 JSON 单行。 */
const inlinePreview = computed(() => {
  const c = current.value;
  if (!c) return "";
  const obvious = pickObvious(c.input);
  if (obvious) return obvious;
  try {
    const text = JSON.stringify(c.input);
    if (!text || text === "{}") return "";
    return text.length > 160 ? `${text.slice(0, 160)}…` : text;
  } catch {
    return "";
  }
});

const inputJson = computed(() => {
  const c = current.value;
  if (!c) return "";
  try {
    return JSON.stringify(c.input, null, 2);
  } catch {
    return String(c.input);
  }
});

const subtitle = computed(() => {
  const c = current.value;
  if (!c) return "";
  const bits: string[] = [];
  if (c.description?.trim()) bits.push(c.description.trim());
  if (c.blockedPath?.trim()) bits.push(`涉及路径：${c.blockedPath.trim()}`);
  if (c.decisionReason?.trim()) bits.push(`触发原因：${c.decisionReason.trim()}`);
  return bits.join(" · ");
});

function pickObvious(input: Record<string, unknown> | null | undefined): string {
  if (!input || typeof input !== "object") return "";
  // 这些字段在 Claude 内置工具里语义上最贴近"这次调用要做什么"
  const candidates = ["command", "file_path", "path", "url", "pattern", "query"];
  for (const key of candidates) {
    const v = (input as Record<string, unknown>)[key];
    if (typeof v === "string" && v.trim()) {
      const text = v.trim();
      return text.length > 160 ? `${text.slice(0, 160)}…` : text;
    }
  }
  return "";
}

async function decide(decision: "allow" | "deny") {
  const c = current.value;
  if (!c || submitting.value) return;
  submitting.value = decision;
  try {
    await respondConsent(
      c.taskId,
      c.requestId,
      decision,
      decision === "deny" ? "用户拒绝了此次工具调用" : undefined,
    );
  } catch (err) {
    console.error("[tool-consent] respond failed", err);
  } finally {
    submitting.value = null;
    expanded.value = false;
  }
}
</script>

<template>
  <Transition name="tool-consent">
    <div
      v-if="current"
      class="tool-consent"
      :class="{ 'tool-consent--danger': danger, 'is-expanded': expanded }"
      role="alert"
      aria-live="assertive"
    >
      <div class="tool-consent__row">
        <span class="tool-consent__icon" aria-hidden="true">
          <AlertTriangle v-if="danger" :size="14" />
          <component v-else :is="toolIcon" :size="14" />
        </span>

        <div class="tool-consent__main">
          <div class="tool-consent__head">
            <span class="tool-consent__tool">{{ current.toolName }}</span>
            <span class="tool-consent__headline">{{ headline }}</span>
          </div>
          <p v-if="inlinePreview" class="tool-consent__preview">{{ inlinePreview }}</p>
          <p v-if="subtitle" class="tool-consent__subtitle">{{ subtitle }}</p>
        </div>

        <button
          v-if="inputJson && inputJson !== '{}'"
          type="button"
          class="tool-consent__toggle"
          :aria-expanded="expanded"
          @click="expanded = !expanded"
        >
          <component
            :is="expanded ? ChevronDown : ChevronRight"
            :size="12"
            aria-hidden="true"
          />
          {{ expanded ? "收起" : "查看入参" }}
        </button>

        <div class="tool-consent__actions">
          <button
            type="button"
            class="ghost tool-consent__btn"
            :disabled="submitting !== null"
            @click="decide('deny')"
          >
            {{ submitting === "deny" ? "处理中…" : "拒绝" }}
          </button>
          <button
            type="button"
            class="tool-consent__btn"
            :class="danger ? 'ghost danger' : 'primary'"
            :disabled="submitting !== null"
            @click="decide('allow')"
          >
            {{ submitting === "allow" ? "处理中…" : danger ? "允许执行" : "允许" }}
          </button>
        </div>
      </div>

      <pre v-if="expanded" class="tool-consent__details">{{ inputJson }}</pre>
    </div>
  </Transition>
</template>
