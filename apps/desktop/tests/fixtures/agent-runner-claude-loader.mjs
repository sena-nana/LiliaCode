const CLAUDE_SDK_URL = "mock:lilia-claude-agent-sdk";

export async function resolve(specifier, context, nextResolve) {
  if (specifier === "@anthropic-ai/claude-agent-sdk") {
    return {
      url: CLAUDE_SDK_URL,
      shortCircuit: true,
    };
  }

  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (url === CLAUDE_SDK_URL) {
    return {
      format: "module",
      shortCircuit: true,
      source: `
        export async function* query() {
          yield {
            type: "stream_event",
            session_id: "mock-session",
            uuid: "stream-1",
            event: {
              type: "content_block_delta",
              delta: { type: "text_delta", text: "你" },
            },
          };
          yield {
            type: "stream_event",
            session_id: "mock-session",
            uuid: "stream-2",
            event: {
              type: "content_block_delta",
              delta: { type: "text_delta", text: "好" },
            },
          };
          yield {
            type: "result",
            session_id: "mock-session",
            uuid: "result-1",
            subtype: "success",
            is_error: false,
            result: "你好",
          };
        }
      `,
    };
  }

  return nextLoad(url, context);
}
