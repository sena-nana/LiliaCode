import {
  sanitizeTimelinePayload,
  shortText,
  stringOrNull,
  toJsonSafe,
} from "./utils.mjs";

export function createProtocolEmitter({ write } = {}) {
  const writer = typeof write === "function"
    ? write
    : (line) => process.stdout.write(line);

  function emit(obj) {
    let line;
    try {
      line = JSON.stringify(obj);
    } catch {
      line = JSON.stringify(toJsonSafe(obj));
    }
    writer(line + "\n");
  }

  function emitTimeline(input) {
    if (!input || typeof input !== "object") return;
    const kind = stringOrNull(input.kind);
    if (!kind) return;

    const status = stringOrNull(input.status) || "info";
    const title = shortText(input.title, 200) || kind;
    const summary = shortText(input.summary, 1200) || "";
    const payload = sanitizeTimelinePayload(input.payload);
    const sourceId = stringOrNull(input.sourceId);
    if (
      payload &&
      typeof payload === "object" &&
      !Array.isArray(payload) &&
      input.payload &&
      typeof input.payload === "object" &&
      !Array.isArray(input.payload) &&
      input.payload.history === true
    ) {
      for (const key of ["threadId", "turnId", "itemId"]) {
        const value = stringOrNull(input.payload[key]);
        if (value) payload[key] = value;
      }
    }
    const event = {
      kind,
      status,
      title,
      summary,
      payload: payload === undefined ? {} : payload,
    };
    if (sourceId) event.sourceId = sourceId;
    const turnIdOverride = stringOrNull(input.turnIdOverride);
    if (turnIdOverride) event.turnIdOverride = turnIdOverride;
    if (typeof input.createdAt === "number" && Number.isFinite(input.createdAt)) {
      event.createdAt = Math.trunc(input.createdAt);
    }
    if (typeof input.updatedAt === "number" && Number.isFinite(input.updatedAt)) {
      event.updatedAt = Math.trunc(input.updatedAt);
    }
    emit({ type: "timeline", event });
  }

  function emitError(message, payload) {
    emit({ type: "error", message, ...(payload ? { payload } : {}) });
  }

  function emitAssistantMessageTimeline(text, status, backend = "assistant") {
    const content = typeof text === "string" ? text : "";
    emitTimeline({
      kind: "message",
      status,
      title: "Assistant",
      summary: content,
      payload: {
        role: "assistant",
        content,
        backend,
      },
      sourceId: `${backend}:text:message`,
    });
  }

  return { emit, emitTimeline, emitError, emitAssistantMessageTimeline };
}

export function claudeTextFragmentSourceId(sessionId, blockKey) {
  return `${sessionId || "claude"}:text:${blockKey}`;
}

export function emitAssistantTextFragmentTimeline(protocol, text, status, sessionId, blockKey) {
  const content = typeof text === "string" ? text : "";
  protocol.emitTimeline({
    kind: "message",
    status,
    title: "Assistant",
    summary: content,
    payload: {
      role: "assistant",
      content,
      backend: "claude",
      sessionId,
      blockKey,
    },
    sourceId: claudeTextFragmentSourceId(sessionId, blockKey),
  });
}

export function createTextPacer({ intervalMs = 33, flushDivisor = 6, emit }) {
  let buffer = "";
  let committed = "";
  let timer = null;

  const tick = () => {
    if (!buffer) {
      clearInterval(timer);
      timer = null;
      return;
    }
    const take = Math.max(1, Math.ceil(buffer.length / flushDivisor));
    committed += buffer.slice(0, take);
    buffer = buffer.slice(take);
    emit(committed);
  };

  return {
    push(delta) {
      if (!delta) return;
      buffer += delta;
      if (!timer) timer = setInterval(tick, intervalMs);
    },
    finishImmediate() {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
      if (buffer) {
        committed += buffer;
        buffer = "";
        emit(committed);
      }
    },
    syncTo(fullText) {
      if (typeof fullText !== "string") return;
      if (fullText.length <= committed.length) return;
      const extra = fullText.slice(committed.length);
      buffer += extra;
      if (!timer) timer = setInterval(tick, intervalMs);
    },
  };
}

export function createSnapshotPacer({ intervalMs = 33, emit }) {
  let pending = null;
  let timer = null;
  let lastEmitAt = 0;

  const flush = () => {
    timer = null;
    if (pending === null) return;
    const snapshot = pending;
    pending = null;
    lastEmitAt = Date.now();
    emit(snapshot);
  };

  return {
    push(snapshot) {
      pending = snapshot;
      if (timer) return;
      const elapsed = Date.now() - lastEmitAt;
      if (elapsed >= intervalMs) flush();
      else timer = setTimeout(flush, intervalMs - elapsed);
    },
    finishImmediate() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      if (pending !== null) {
        const snapshot = pending;
        pending = null;
        lastEmitAt = Date.now();
        emit(snapshot);
      }
    },
    cancel() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      pending = null;
    },
  };
}
