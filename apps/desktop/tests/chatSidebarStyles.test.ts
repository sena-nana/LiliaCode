import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync("src/styles.css", "utf8");
const taskDetail = readFileSync("src/pages/TaskDetail.vue", "utf8");
const titleBar = readFileSync("src/components/TitleBar.vue", "utf8");
const chatSidebarHost = readFileSync("src/components/chat/ChatSidebarHost.vue", "utf8");

function selectorIndex(selector: string): number {
  const match = new RegExp(`(^|\\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`).exec(styles);
  return match?.index ?? -1;
}

function rule(selector: string): string {
  const index = selectorIndex(selector);
  expect(index).toBeGreaterThan(-1);
  const end = styles.indexOf("}", index);
  return styles.slice(index, end + 1);
}

describe("chat sidebar styles", () => {
  it("对话页使用右侧栏布局并保留主对话伸缩空间", () => {
    const layout = rule(".chat-layout {");
    const main = rule(".chat-layout__main {");
    const transcript = rule(".chat-layout__main > :where(.chat-transcript-frame) {");

    expect(taskDetail).toContain('class="chat-layout"');
    expect(taskDetail).not.toContain("is-sidebar-open");
    expect(taskDetail).not.toContain("useChatSidebar");
    expect(taskDetail).toContain("<ChatSidebarHost");
    expect(taskDetail).not.toContain("chat-sidebar-toggle");
    expect(titleBar).toContain("titlebar__chat-sidebar-btn");
    expect(titleBar).toContain("打开对话侧栏");
    expect(chatSidebarHost).toContain('aria-label="对话侧栏"');
    expect(chatSidebarHost).not.toContain("chat-sidebar__close");
    expect(chatSidebarHost).not.toContain("chat-sidebar__title");
    expect(chatSidebarHost).not.toContain("activeTitle");
    expect(layout).toContain("--chat-sidebar-width: clamp(280px, 28vw, 360px)");
    expect(layout).toContain("--chat-sidebar-easing: cubic-bezier(0.4, 0, 0.2, 1)");
    expect(layout).toContain("display: flex");
    expect(layout).toContain("overflow: hidden");
    expect(main).toContain("flex: 1 1 auto");
    expect(main).toContain("min-width: 0");
    expect(transcript).toContain("flex: 1 1 auto");
  });

  it("关闭态宽度为 0，开启态用侧栏宽度挤压主内容", () => {
    const sidebar = rule(".chat-sidebar {");
    const sidebarOpen = rule(".chat-sidebar.is-open {");

    expect(sidebar).toContain("flex: 0 0 0");
    expect(sidebar).toContain("width: 0");
    expect(sidebar).toContain("opacity: 0");
    expect(sidebar).toContain("pointer-events: none");
    expect(sidebar).toContain("overflow: hidden");
    expect(sidebarOpen).toContain("flex-basis: var(--chat-sidebar-width)");
    expect(sidebarOpen).toContain("width: var(--chat-sidebar-width)");
    expect(sidebarOpen).toContain("opacity: 1");
    expect(sidebarOpen).toContain("pointer-events: auto");
  });

  it("侧栏开关有渐入渐出动画，且不被 reduced-motion 偏好关闭", () => {
    const sidebar = rule(".chat-sidebar {");

    expect(sidebar).toContain("transition:");
    expect(sidebar).toContain("flex-basis 0.26s var(--chat-sidebar-easing)");
    expect(sidebar).toContain("width 0.26s var(--chat-sidebar-easing)");
    expect(sidebar).toContain("margin-left 0.26s var(--chat-sidebar-easing)");
    expect(sidebar).toContain("opacity 0.22s var(--chat-sidebar-easing)");
    expect(sidebar).toContain("transform 0.26s var(--chat-sidebar-easing)");
    expect(styles).not.toContain(".titlebar:hover .titlebar__chat-sidebar-btn");
    expect(styles).not.toContain(".titlebar:focus-within .titlebar__chat-sidebar-btn");
    expect(rule(".titlebar__btn {")).toContain("transition:");
    expect(styles).not.toContain("@media (prefers-reduced-motion: reduce) {\n  .chat-sidebar");
  });
});
