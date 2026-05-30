import { computed, type Component, type ComputedRef } from "vue";
import {
  Bot,
  FilePen,
  Globe,
  Search,
  Terminal,
  Wrench,
} from "lucide-vue-next";
import type { ToolConsentRequest } from "../services/chat";

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

export function useToolConsentPresentation(
  request: ComputedRef<ToolConsentRequest | null>,
) {
  const toolDanger = computed(() => {
    const c = request.value;
    if (!c || c.toolName !== "Bash") return false;
    const cmd = c.input.command;
    return typeof cmd === "string" && DANGEROUS_BASH_RE.test(cmd);
  });

  const toolIcon = computed<Component>(() => {
    const name = request.value?.toolName ?? "";
    return TOOL_ICON_MAP[name] ?? Wrench;
  });

  const toolHeadline = computed(() => {
    const c = request.value;
    if (!c) return "";
    const tool = c.displayName?.trim() || c.toolName || "工具";
    if (c.title?.trim()) return c.title.trim();
    return toolDanger.value ? `想执行 ${tool}` : `想使用 ${tool}`;
  });

  const toolInlinePreview = computed(() => {
    const c = request.value;
    if (!c) return "";
    const obvious = pickObvious(c.input);
    if (obvious) return obvious;
    try {
      const text = JSON.stringify(c.input);
      if (!text || text === "{}") return "";
      return text.length > 160 ? `${text.slice(0, 160)}...` : text;
    } catch {
      return "";
    }
  });

  const toolInputJson = computed(() => {
    const c = request.value;
    if (!c) return "";
    try {
      return JSON.stringify(c.input, null, 2);
    } catch {
      return String(c.input);
    }
  });

  const toolSubtitle = computed(() => {
    const c = request.value;
    if (!c) return "";
    return [
      c.description?.trim(),
      c.blockedPath?.trim() ? `涉及路径：${c.blockedPath.trim()}` : "",
      c.decisionReason?.trim() ? `触发原因：${c.decisionReason.trim()}` : "",
    ].filter(Boolean).join(" · ");
  });

  return {
    toolDanger,
    toolIcon,
    toolHeadline,
    toolInlinePreview,
    toolInputJson,
    toolSubtitle,
  };
}

function pickObvious(input: Record<string, unknown> | null | undefined): string {
  if (!input || typeof input !== "object") return "";
  const candidates = ["command", "file_path", "path", "url", "pattern", "query"];
  for (const key of candidates) {
    const v = input[key];
    if (typeof v === "string" && v.trim()) {
      const text = v.trim();
      return text.length > 160 ? `${text.slice(0, 160)}...` : text;
    }
  }
  return "";
}
