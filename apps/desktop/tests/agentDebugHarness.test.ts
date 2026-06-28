import { afterEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

const originalConsoleError = console.error;
const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;

function testRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: { template: "<div />" } }],
  });
}

describe("agent debug harness", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
    document.body.innerHTML = "";
    console.error = originalConsoleError;
    HTMLElement.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    delete (window as typeof window & { __liliaAgentDebug?: unknown }).__liliaAgentDebug;
  });

  it("does not install unless the dev env flag is enabled", async () => {
    vi.stubEnv("VITE_LILIA_AGENT_DEBUG", "0");
    const { installAgentDebugHarness } = await import("../src/agentDebug/harness");

    installAgentDebugHarness(testRouter());

    expect((window as typeof window & { __liliaAgentDebug?: unknown }).__liliaAgentDebug).toBeUndefined();
  });

  it("collects data-agent-id elements in an observe snapshot", async () => {
    vi.stubEnv("VITE_LILIA_AGENT_DEBUG", "1");
    document.body.innerHTML = `
      <button data-agent-id="project.create.button">新建项目</button>
      <input data-agent-id="project.name.input" value="" />
    `;
    const { installAgentDebugHarness } = await import("../src/agentDebug/harness");

    installAgentDebugHarness(testRouter());
    const snapshot = (window as typeof window & {
      __liliaAgentDebug: { observe: () => unknown };
    }).__liliaAgentDebug.observe() as { elements: Array<{ id: string; role: string; enabled: boolean }> };

    expect(snapshot.elements).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: "project.create.button", role: "button", enabled: true }),
        expect.objectContaining({ id: "project.name.input", role: "input", enabled: true }),
      ]),
    );
  });

  it("reports visible interactive elements that are missing stable agent ids", async () => {
    vi.stubEnv("VITE_LILIA_AGENT_DEBUG", "1");
    HTMLElement.prototype.getBoundingClientRect = vi.fn(() => ({
      x: 10,
      y: 20,
      width: 120,
      height: 32,
      top: 20,
      right: 130,
      bottom: 52,
      left: 10,
      toJSON: () => ({}),
    }));
    document.body.innerHTML = `
      <button aria-label="未标记按钮">未标记</button>
      <button data-agent-id="marked.button">已标记</button>
    `;
    const { installAgentDebugHarness } = await import("../src/agentDebug/harness");

    installAgentDebugHarness(testRouter());
    const snapshot = (window as typeof window & {
      __liliaAgentDebug: { observe: () => unknown };
    }).__liliaAgentDebug.observe() as {
      missingAgentIds: Array<{ role: string; text: string; tagName: string; nearestAgentId: string | null }>;
    };

    expect(snapshot.missingAgentIds).toEqual([
      expect.objectContaining({
        role: "button",
        text: "未标记按钮",
        tagName: "button",
        nearestAgentId: null,
      }),
    ]);
  });

  it("acts on click, focus, and type targets", async () => {
    vi.stubEnv("VITE_LILIA_AGENT_DEBUG", "1");
    const clicked = vi.fn();
    document.body.innerHTML = `
      <button data-agent-id="chat.composer.send">发送</button>
      <textarea data-agent-id="chat.composer.input"></textarea>
    `;
    document.querySelector("button")?.addEventListener("click", clicked);
    const { installAgentDebugHarness } = await import("../src/agentDebug/harness");

    installAgentDebugHarness(testRouter());
    const api = (window as typeof window & {
      __liliaAgentDebug: {
        act: (action:
          | { type: "click"; target: string }
          | { type: "focus"; target: string }
          | { type: "type"; target: string; text: string }
        ) => Promise<unknown>;
      };
    }).__liliaAgentDebug;

    await api.act({ type: "click", target: "chat.composer.send" });
    await api.act({ type: "focus", target: "chat.composer.input" });
    await api.act({ type: "type", target: "chat.composer.input", text: "hello" });

    expect(clicked).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(document.querySelector("[data-agent-id='chat.composer.input']"));
    expect(document.querySelector<HTMLTextAreaElement>("[data-agent-id='chat.composer.input']")?.value).toBe("hello");
  });
});

