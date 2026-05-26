<script setup lang="ts">
/**
 * 工具调用授权 inline 卡片。挂在 ChatComposer 上方、贴着 .chat 网格的最后一行。
 *
 * 视觉与 ChatComposer / TodoFloat 共用一套几何（bg-elev + 14px 圆角 + 软阴影），
 * 不再用 modal 弹窗——授权交互是日常高频动作，弹窗会打断聊天节奏。
 *
 * 设计点：
 * - 危险工具（Bash / Write / Edit 等）给 .tool-consent--danger：左侧 3px 红条 +
 *   主按钮换 ghost danger。其它工具走 accent 主按钮。
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

const DANGEROUS_TOOLS = new Set([
  "Bash",
  "Write",
  "Edit",
  "MultiEdit",
  "NotebookEdit",
  "WebFetch",
]);

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
  const name = current.value?.toolName ?? "";
  return DANGEROUS_TOOLS.has(name);
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
