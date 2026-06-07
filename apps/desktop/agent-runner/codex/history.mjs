import { createCodexAppServer } from "./appServer.mjs";
import { initializeCodexAppServer } from "./runCodex.mjs";
import { codexHistoryTimelineEvents } from "./timeline.mjs";
import { isRecord, shortText, stringOrNull } from "../utils.mjs";

const DEFAULT_LIMIT = 20;
const DEFAULT_TURN_LIMIT = 50;

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
  const id = firstString(row?.id, row?.threadId, row?.thread?.id);
  if (!id) return null;
  const title = firstString(
    row.title,
    row.name,
    row.summary,
    row.thread?.title,
    previewFromTurns(turns),
  ) || "Codex thread";
  return {
    id,
    title: shortText(title, 160) || "Codex thread",
    status: firstString(row.status, row.thread?.status),
    model: firstString(row.model, row.thread?.model),
    sourceKind: firstString(row.sourceKind, row.source_kind, row.source?.kind),
    createdAt: millisFromSeconds(row.createdAt ?? row.created_at ?? row.thread?.createdAt),
    updatedAt: millisFromSeconds(
      row.updatedAt ??
        row.updated_at ??
        row.lastUpdatedAt ??
        row.thread?.updatedAt ??
        row.thread?.lastUpdatedAt,
    ),
    archived: row.archived === true || row.thread?.archived === true,
    preview: firstString(row.preview, row.description, previewFromTurns(turns)),
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

export async function readCodexThreadTurns(server, threadId, { limit = DEFAULT_TURN_LIMIT } = {}) {
  const result = await server.request("thread/turns/list", {
    threadId,
    limit,
    sortDirection: "asc",
    itemsView: "full",
  });
  const turns = readArray(result?.data || result?.turns);
  const out = [];
  for (const turn of turns) {
    out.push(needsItemBackfill(turn) ? await readTurnItems(server, threadId, turn) : turn);
  }
  return {
    turns: out,
    nextCursor: stringOrNull(result?.nextCursor) || stringOrNull(result?.next_cursor),
  };
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
    if (searchTerm) params.searchTerm = searchTerm;
    if (cursor) params.cursor = cursor;
    return normalizeSearchResult(await server.request("thread/search", params));
  } finally {
    server.close();
  }
}

export async function previewCodexThread(threadId, { createServer = createCodexAppServer } = {}) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const { turns } = await readCodexThreadTurns(server, threadId, { limit: 12 });
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

export async function syncCodexThreadHistoryForTask(
  { taskId, threadId, limit = DEFAULT_TURN_LIMIT },
  { createServer = createCodexAppServer } = {},
) {
  const server = createServer();
  try {
    await initializeCodexAppServer(server);
    const { turns, nextCursor } = await readCodexThreadTurns(server, threadId, { limit });
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
