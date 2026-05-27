import { describe, expect, it, vi } from "vitest";
import {
  appendClaudeBlockText,
  CLAUDE_BLOCK_TYPES,
  closeClaudeBlock,
  createClaudeStreamState,
  dispatchClaudeStreamEvent,
  extractClaudeBlockInitialText,
  extractClaudeTextDeltaText,
  extractClaudeThinkingDeltaText,
  finalizeClaudeReasoningBlocks,
  getClaudeBlockText,
  getClaudeBlockType,
  openClaudeBlock,
} from "../agent-runner/claudeStream.mjs";

type StreamEvent = Record<string, unknown>;

function startBlock(index: number, blockType: string, extra: Record<string, unknown> = {}): StreamEvent {
  return {
    type: "content_block_start",
    index,
    content_block: { type: blockType, ...extra },
  };
}
function stopBlock(index: number): StreamEvent {
  return { type: "content_block_stop", index };
}
function thinkingDelta(index: number, text: string): StreamEvent {
  return {
    type: "content_block_delta",
    index,
    delta: { type: "thinking_delta", thinking: text },
  };
}
function textDelta(index: number, text: string): StreamEvent {
  return {
    type: "content_block_delta",
    index,
    delta: { type: "text_delta", text },
  };
}

function runDispatcher(events: StreamEvent[]) {
  const state = createClaudeStreamState();
  const textChunks: string[] = [];
  const reasoningSnapshots: Array<{ index: number; text: string; blockType: string }> = [];
  const onTextDelta = vi.fn((t: string) => {
    textChunks.push(t);
  });
  const onReasoning = vi.fn((info: { index: number; text: string; blockType: string }) => {
    reasoningSnapshots.push({ index: info.index, text: info.text, blockType: info.blockType });
  });
  for (const event of events) {
    dispatchClaudeStreamEvent({ event, state, onTextDelta, onReasoning });
  }
  return { state, textChunks, reasoningSnapshots, onTextDelta, onReasoning };
}

describe("dispatchClaudeStreamEvent", () => {
  it("fixture A：thinking → text 完整序列时两通道互不串字", () => {
    const thinkingParts = ["我先想想…", "也许应该", "拆成两步。"];
    const textParts = ["第一步：", "读文件；", "第二步：写文件。"];

    const { textChunks, reasoningSnapshots } = runDispatcher([
      { type: "message_start" },
      startBlock(0, CLAUDE_BLOCK_TYPES.THINKING),
      ...thinkingParts.map((t) => thinkingDelta(0, t)),
      stopBlock(0),
      startBlock(1, CLAUDE_BLOCK_TYPES.TEXT),
      ...textParts.map((t) => textDelta(1, t)),
      stopBlock(1),
    ]);

    expect(textChunks).toEqual(textParts);

    // 每条 thinking_delta 都产生一条累积快照。
    expect(reasoningSnapshots).toHaveLength(thinkingParts.length);
    const accumulated = thinkingParts.reduce<string[]>((acc, part) => {
      acc.push((acc[acc.length - 1] || "") + part);
      return acc;
    }, []);
    expect(reasoningSnapshots.map((s) => s.text)).toEqual(accumulated);

    // 关键不变量：reasoning 通道任何快照都不能含 text_delta 的文本。
    const allReasoningText = reasoningSnapshots.map((s) => s.text).join("\n");
    for (const t of textParts) {
      expect(allReasoningText).not.toContain(t);
    }
  });

  it("fixture B：未知 block 类型两边都不路由，不抛错", () => {
    const { textChunks, reasoningSnapshots } = runDispatcher([
      startBlock(0, "future_unknown_block_type"),
      textDelta(0, "看上去像 text 但实际不是"),
      thinkingDelta(0, "也不该当 thinking"),
      stopBlock(0),
    ]);

    expect(textChunks).toEqual([]);
    expect(reasoningSnapshots).toEqual([]);
  });

  it("fixture C：text block 中混入伪造的 thinking_delta 不会污染思考缓冲（c23c0ef 反向回归）", () => {
    const { textChunks, reasoningSnapshots, state } = runDispatcher([
      startBlock(0, CLAUDE_BLOCK_TYPES.TEXT),
      textDelta(0, "正常 text"),
      // 模拟 SDK bug：在 text block 内部错发 thinking_delta。
      thinkingDelta(0, "<这段绝不能进 thinking 缓冲>"),
      textDelta(0, "继续正常 text"),
      stopBlock(0),
    ]);

    expect(textChunks).toEqual(["正常 text", "继续正常 text"]);
    expect(reasoningSnapshots).toEqual([]);
    expect(getClaudeBlockText(state, 0)).toBe(""); // text block 不累积 text 到 state（pacer 负责）
  });

  it("fixture D：content_block_start 漏发时 delta 静默丢弃，由上层 result 兜底", () => {
    const { textChunks, reasoningSnapshots, state } = runDispatcher([
      textDelta(0, "孤儿 text_delta"),
      thinkingDelta(0, "孤儿 thinking_delta"),
    ]);

    expect(textChunks).toEqual([]);
    expect(reasoningSnapshots).toEqual([]);
    expect(getClaudeBlockType(state, 0)).toBe("");
  });

  it("redacted_thinking 也走 reasoning 通道", () => {
    const { reasoningSnapshots } = runDispatcher([
      startBlock(0, CLAUDE_BLOCK_TYPES.REDACTED_THINKING),
      thinkingDelta(0, "敏感思考"),
      stopBlock(0),
    ]);

    expect(reasoningSnapshots).toHaveLength(1);
    expect(reasoningSnapshots[0].blockType).toBe(CLAUDE_BLOCK_TYPES.REDACTED_THINKING);
    expect(reasoningSnapshots[0].text).toBe("敏感思考");
  });

  it("content_block_start 含 initial thinking 文本时立刻发一条快照", () => {
    const { reasoningSnapshots } = runDispatcher([
      startBlock(0, CLAUDE_BLOCK_TYPES.THINKING, { thinking: "首段思考" }),
      thinkingDelta(0, "增量"),
    ]);

    expect(reasoningSnapshots).toHaveLength(2);
    expect(reasoningSnapshots[0].text).toBe("首段思考");
    expect(reasoningSnapshots[1].text).toBe("首段思考增量");
  });
});

describe("Claude block state machine", () => {
  it("openClaudeBlock / closeClaudeBlock / appendClaudeBlockText 维护 per-index 状态", () => {
    const state = createClaudeStreamState();
    openClaudeBlock(state, 0, CLAUDE_BLOCK_TYPES.THINKING);
    openClaudeBlock(state, 1, CLAUDE_BLOCK_TYPES.TEXT);

    appendClaudeBlockText(state, 0, "abc");
    appendClaudeBlockText(state, 0, "def");
    appendClaudeBlockText(state, 1, "xyz");

    expect(getClaudeBlockText(state, 0)).toBe("abcdef");
    expect(getClaudeBlockText(state, 1)).toBe("xyz");
    expect(getClaudeBlockType(state, 0)).toBe(CLAUDE_BLOCK_TYPES.THINKING);

    closeClaudeBlock(state, 0);
    expect(getClaudeBlockType(state, 0)).toBe("");
    expect(getClaudeBlockText(state, 0)).toBe("");
  });
});

describe("Claude delta 抽取器", () => {
  it("extractClaudeTextDeltaText 只认 text_delta", () => {
    expect(extractClaudeTextDeltaText(textDelta(0, "hi"))).toBe("hi");
    expect(extractClaudeTextDeltaText(thinkingDelta(0, "x"))).toBeNull();
    expect(extractClaudeTextDeltaText({ type: "message_delta" })).toBeNull();
    expect(extractClaudeTextDeltaText(null as unknown as StreamEvent)).toBeNull();
  });

  it("extractClaudeThinkingDeltaText 只认 thinking_delta", () => {
    expect(extractClaudeThinkingDeltaText(thinkingDelta(0, "想"))).toBe("想");
    expect(extractClaudeThinkingDeltaText(textDelta(0, "x"))).toBeNull();
    expect(
      extractClaudeThinkingDeltaText({
        type: "content_block_delta",
        index: 0,
        delta: { type: "signature_delta", signature: "sig" },
      }),
    ).toBeNull();
  });

  it("extractClaudeBlockInitialText 按 blockType 走对应字段", () => {
    expect(
      extractClaudeBlockInitialText(
        startBlock(0, CLAUDE_BLOCK_TYPES.THINKING, { thinking: "init" }),
        CLAUDE_BLOCK_TYPES.THINKING,
      ),
    ).toBe("init");
    expect(
      extractClaudeBlockInitialText(
        startBlock(0, CLAUDE_BLOCK_TYPES.TEXT, { text: "init-text" }),
        CLAUDE_BLOCK_TYPES.TEXT,
      ),
    ).toBe("init-text");
    // text 块里 thinking 字段不算 initial。
    expect(
      extractClaudeBlockInitialText(
        startBlock(0, CLAUDE_BLOCK_TYPES.TEXT, { thinking: "干扰" }),
        CLAUDE_BLOCK_TYPES.TEXT,
      ),
    ).toBeNull();
  });
});

describe("finalizeClaudeReasoningBlocks", () => {
  it("只回调 reasoning 类 block，回调后 state 清空", () => {
    const state = createClaudeStreamState();
    openClaudeBlock(state, 0, CLAUDE_BLOCK_TYPES.THINKING);
    openClaudeBlock(state, 1, CLAUDE_BLOCK_TYPES.TEXT);
    openClaudeBlock(state, 2, CLAUDE_BLOCK_TYPES.REDACTED_THINKING);
    appendClaudeBlockText(state, 0, "t0");
    appendClaudeBlockText(state, 2, "t2");

    const seen: Array<{ index: number; text: string; blockType: string }> = [];
    finalizeClaudeReasoningBlocks(state, (info) => seen.push(info));

    expect(seen.map((s) => s.index).sort()).toEqual([0, 2]);
    expect(seen.find((s) => s.index === 0)?.text).toBe("t0");
    expect(seen.find((s) => s.index === 2)?.text).toBe("t2");
    expect(state.streamBlocks.size).toBe(0);
  });
});
