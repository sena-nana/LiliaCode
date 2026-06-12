import { createCodexAppServer } from "./appServer.mjs";
import { initializeCodexAppServer } from "./runCodex.mjs";
import { codexHistoryTimelineEvents } from "./timeline.mjs";
import { isRecord, shortText, stringOrNull } from "../utils.mjs";

const DEFAULT_LIMIT = 20;
const DEFAULT_TURN_LIMIT = 50;
const PREVIEW_TURN_LIMIT = 8;
const PREVIEW_MESSAGE_LIMIT = 5;
const ITEM_BACKFILL_CONCURRENCY = 4;

function numberOrNull(value) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function millisFromSeconds(value) {
  const seconds = numberOrNull(value);
  return seconds === null ? null : Math.trunc(seconds * 1000);
}

function readArray(value) {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function firstString(...values) {
  for (const value of values) {
    const text = stringOrNull(value)?.trim();
    if (text) return text;
  }
  return null;
}

function pickMessageText(item) {
  if (!isRecord(item)) return "";
  if (typeof item.text === "string") return item.text;
  if (typeof item.content === "string") return item.content;
  if (Array.isArray(item.content)) {
    return item.content
      .map((part) => {
        if (typeof part === "string") return part;
        if (!isRecord(part)) return "";
        return stringOrNull(part.text) || stringOrNull(part.content) || "";
      })
      .filter(Boolean)
      .join("\n");
  }
  return "";
}

function previewFromTurns(turns) {
  for (let index = turns.length - 1; index >= 0; index -= 1) {
    const items = readArray(turns[index]?.items);
    for (let itemIndex = items.length - 1; itemIndex >= 0; itemIndex -= 1) {
      const item = items[itemIndex];
      if (item.type !== "agentMessage" && item.type !== "userMessage") continue;
      const text = pickMessageText(item);
      if (text.trim()) return shortText(text, 180);
    }
  }
  return null;
}

function normalizeThread(row, turns = []) {
  const thread = isRecord(row?.thread) ? row.thread : {};
  const id = firstString(row?.id, row?.threadId, thread.id);
  if (!id) return null;
  const title = firstString(
    row.title,
    row.name,
    row.summary,
    thread.title,
    thread.name,
    previewFromTurns(turns),
  ) || "Codex thread";
  return {
    id,
    title: shortText(title, 160) || "Codex thread",
    status: firstString(row.status, thread.status),
    model: firstString(row.model, row.modelProvider, thread.model, thread.modelProvider),
    sourceKind: firstString(row.sourceKind, row.source_kind, row.source?.kind),
    createdAt: millisFromSeconds(row.createdAt ?? row.created_at ?? thread.createdAt),
    updatedAt: millisFromSeconds(
      row.updatedAt ??
        row.updated_at ??
        row.lastUpdatedAt ??
        thread.updatedAt ??
        thread.lastUpdatedAt,
    ),
    archived: row.archived === true,
    preview: firstString(row.snippet, row.preview, row.description, thread.preview, previewFromTurns(turns)),
  };
}

function normalizeSearchResult(result) {
  const data = readArray(result?.data || result?.threads || result?.items);
  const threads = data
    .map((row) => normalizeThread(row))
    .filter(Boolean);
  return {
    threads,
    nextCursor: stringOrNull(result?.nextCursor) || stringOrNull(result?.next_cursor),
  };
}

function needsItemBackfill(turn) {
  if (!isRecord(turn)) return false;
  if (!Array.isArray(turn.items)) return true;
  return turn.itemsTruncated === true ||
    turn.items_truncated === true ||
    turn.hasMoreItems === true ||
    turn.has_more_items === true;
}

async function readTurnItems(server, threadId, turn) {
  const turnId = stringOrNull(turn?.id);
  if (!threadId || !turnId) return turn;
  try {
    const result = await server.request("thread/turns/items/list", {
      threadId,
      turnId,
      limit: 200,
      sortDirection: "asc",
    });
    const items = readArray(result?.data || result?.items);
    return { ...turn, items };
  } catch {
    return turn;
  }
}

async function mapWithConcurrency(items, limit, mapper) {
  const out = new Array(items.length);
  let index = 0;
  const workerCount = Math.max(1, Math.min(limit, items.length));
  await Promise.all(Array.from({ length: workerCount }, async () => {
    while (index < items.length) {
      const current = index;
      index += 1;
      out[current] = await mapper(items[current], current);
    }
  }));
  return out;
}

export async function readCodexThreadTurns(
  server,
  threadId,
  {
    limit = DEFAULT_TURN_LIMIT,
    sortDirection = "asc",
    backfillConcurrency = ITEM_BACKFILL_CONCURRENCY,
    cursor = null,
  } = {},
) {
  const params = {
    threadId,
    limit,
    sortDirection,
    itemsView: "full",
  };
  const trimmedCursor = stringOrNull(cursor)?.trim();
  if (trimmedCursor) params.cursor = trimmedCursor;
  const result = await server.request("thread/turns/list", params);
  const turns = readArray(result?.data || result?.turns);
  const out = await mapWithConcurrency(
    turns,
    backfillConcurrency,
    (turn) => needsItemBackfill(turn) ? readTurnItems(server, threadId, turn) : turn,
  );
  return {
    turns: out,
    nextCursor: stringOrNull(result?.nextCursor) || stringOrNull(result?.next_cursor),
  };
}

async function readAllCodexThreadTurns(server, threadId, { limit = DEFAULT_TURN_LIMIT } = {}) {
  const turns = [];
  let cursor = null;
  do {
    const previousCursor = cursor;
    const result = await readCodexThreadTurns(server, threadId, { limit, cursor });
    turns.push(...result.turns);
    cursor = stringOrNull(result.nextCursor)?.trim() || null;
    if (cursor && cursor === previousCursor) {
      throw new Error("Codex history preview 返回了重复 cursor，已停止读取。");
    }
  } while (cursor);
  return turns;
}

export function codexHistoryTimelineInputs(taskId, threadId, turns) {
  return codexHistoryTimelineEvents(threadId, turns).map((event) => ({
    id: event.sourceId ? `${taskId}:${event.turnIdOverride || event.turnId || "history"}:${event.sourceId}` : null,
    taskId,
    turnId: event.turnIdOverride || null,
    backend: "codex",
    kind: event.kind,
    status: event.status,
    title: event.title,
    summary: event.summary || null,
    payload: event.payload || {},
    createdAt: event.createdAt ?? null,
    updatedAt: event.updatedAt ?? null,
  }));
}

export async function searchCodexThreads(input = {}, { createServer = createCodexAppServer } = {}) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const params = {
      limit: Math.max(1, Math.min(50, Number(input.limit) || DEFAULT_LIMIT)),
      sortDirection: "desc",
      archived: input.archived === true,
    };
    const searchTerm = stringOrNull(input.searchTerm)?.trim();
    const cursor = stringOrNull(input.cursor)?.trim();
    if (cursor) params.cursor = cursor;
    if (searchTerm) {
      params.searchTerm = searchTerm;
      return normalizeSearchResult(await server.request("thread/search", params));
    }
    return normalizeSearchResult(await server.request("thread/list", params));
  } finally {
    server.close();
  }
}

export async function cleanCodexThreadBackgroundTerminals(
  threadId,
  { createServer = createCodexAppServer } = {},
) {
  const trimmed = stringOrNull(threadId)?.trim();
  if (!trimmed) throw new Error("Codex threadId is required");
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    await server.request("thread/backgroundTerminals/clean", { threadId: trimmed });
    return { threadId: trimmed, cleaned: true };
  } finally {
    server.close();
  }
}

export async function archiveCodexThread(
  threadId,
  { createServer = createCodexAppServer } = {},
) {
  const trimmed = stringOrNull(threadId)?.trim();
  if (!trimmed) throw new Error("Codex threadId is required");
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    await server.request("thread/archive", { threadId: trimmed });
    return { threadId: trimmed, archived: true };
  } finally {
    server.close();
  }
}

export async function renameCodexThread(
  threadId,
  name,
  { createServer = createCodexAppServer } = {},
) {
  const trimmedThreadId = stringOrNull(threadId)?.trim();
  const trimmedName = stringOrNull(name)?.trim();
  if (!trimmedThreadId) throw new Error("Codex threadId is required");
  if (!trimmedName) throw new Error("Codex thread name is required");
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    await server.request("thread/name/set", {
      threadId: trimmedThreadId,
      name: trimmedName,
    });
    return { threadId: trimmedThreadId, name: trimmedName, renamed: true };
  } finally {
    server.close();
  }
}

export async function previewCodexThread(threadId, { createServer = createCodexAppServer } = {}) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const turns = await readAllCodexThreadTurns(server, threadId);
    const events = codexHistoryTimelineInputs("preview", threadId, turns);
    const thread = normalizeThread({ id: threadId }, turns);
    return {
      thread,
      events,
      eventCount: events.length,
    };
  } finally {
    server.close();
  }
}

function previewRoleForItem(item) {
  if (item?.type === "userMessage") return "user";
  if (item?.type === "agentMessage") return "assistant";
  return null;
}

function previewEventCountFromTurns(turns) {
  return turns.reduce((count, turn) => count + readArray(turn?.items).filter((item) => {
    switch (item.type) {
      case "agentMessage":
      case "userMessage":
      case "reasoning":
      case "commandExecution":
      case "fileChange":
      case "mcpToolCall":
      case "webSearch":
      case "plan":
        return true;
      default:
        return false;
    }
  }).length, 0);
}

async function backfillTurnsUntilMessages(server, threadId, turns, messageLimit) {
  const out = turns.slice();
  const found = [];
  for (let index = 0; index < out.length && found.length < messageLimit; index += 1) {
    if (needsItemBackfill(out[index])) {
      out[index] = await readTurnItems(server, threadId, out[index]);
    }
    const items = readArray(out[index]?.items);
    for (let itemIndex = items.length - 1; itemIndex >= 0 && found.length < messageLimit; itemIndex -= 1) {
      if (previewRoleForItem(items[itemIndex])) found.push(items[itemIndex]);
    }
  }
  return out;
}

function previewMessagesFromTurns(turns, messageLimit = PREVIEW_MESSAGE_LIMIT) {
  const messages = [];
  for (const turn of turns) {
    for (const item of readArray(turn?.items)) {
      const role = previewRoleForItem(item);
      if (!role) continue;
      messages.push({
        id: firstString(item.id, item.clientId) || `${role}:${messages.length}`,
        role,
        summary: shortText(pickMessageText(item), 1200) || null,
      });
    }
  }
  return messages.slice(-messageLimit);
}

export async function previewCodexThreadLite(threadId, { createServer = createCodexAppServer } = {}) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const result = await server.request("thread/turns/list", {
      threadId,
      limit: PREVIEW_TURN_LIMIT,
      sortDirection: "desc",
      itemsView: "full",
    });
    const recentTurns = await backfillTurnsUntilMessages(
      server,
      threadId,
      readArray(result?.data || result?.turns),
      PREVIEW_MESSAGE_LIMIT,
    );
    const turns = recentTurns.slice().reverse();
    const messages = previewMessagesFromTurns(turns);
    return {
      thread: normalizeThread({ id: threadId }, turns),
      eventCount: previewEventCountFromTurns(turns),
      messages,
      hasFullPreview: true,
    };
  } finally {
    server.close();
  }
}

export async function syncCodexThreadHistoryForTask(
  { taskId, threadId, limit = DEFAULT_TURN_LIMIT, cursor = null },
  { createServer = createCodexAppServer } = {},
) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const { turns, nextCursor } = await readCodexThreadTurns(server, threadId, { limit, cursor });
    const events = codexHistoryTimelineInputs(taskId, threadId, turns);
    const thread = normalizeThread({ id: threadId }, turns);
    return {
      thread,
      events,
      eventCount: events.length,
      nextCursor,
    };
  } finally {
    server.close();
  }
}
