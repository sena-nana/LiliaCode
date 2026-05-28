import type { AgentTimelineEvent, AgentTimelineEventStatus } from "@lilia/contracts";
import {
  aggregateTimelineStatus,
  timelineDeclaredGroupUnit,
  timelineGroupKey,
} from "./timelineDisplay";

export type TimelineEventEntry = {
  type: "event";
  id: string;
  createdAt: number;
  turnSeq: number;
  intraTurnOrder: number;
  event: AgentTimelineEvent;
  processEvents?: AgentTimelineEvent[];
  isProcessChild?: boolean;
};

export type TimelineGroupEntry = {
  type: "group";
  id: string;
  createdAt: number;
  turnSeq: number;
  intraTurnOrder: number;
  groupKey: string;
  events: AgentTimelineEvent[];
  representative: AgentTimelineEvent;
  aggregatedStatus: AgentTimelineEventStatus;
  groupCount: number;
  isProcessChild?: boolean;
};

export type TimelineEntry = TimelineEventEntry | TimelineGroupEntry;

export function mergeAdjacentTimelineGroups(entries: TimelineEventEntry[]): TimelineEntry[] {
  const result: TimelineEntry[] = [];
  let bucket: TimelineEventEntry[] = [];
  let bucketKey: string | null = null;

  const flush = () => {
    if (bucket.length === 0) return;
    if (bucket.length === 1) {
      result.push(...bucket);
    } else {
      const first = bucket[0];
      const events = bucket.map((b) => b.event);
      result.push({
        type: "group",
        id: `group:${bucketKey}:${first.event.id}`,
        createdAt: first.createdAt,
        turnSeq: first.turnSeq,
        intraTurnOrder: first.intraTurnOrder,
        groupKey: bucketKey!,
        events,
        representative: first.event,
        aggregatedStatus: aggregateTimelineStatus(events),
        groupCount: events.reduce((total, event) => total + (timelineDeclaredGroupUnit(event)?.count ?? 1), 0),
        isProcessChild: first.isProcessChild,
      });
    }
    bucket = [];
    bucketKey = null;
  };

  for (const entry of entries) {
    const event = entry.event;
    const key = event.kind === "message" ? null : timelineGroupKey(event);

    if (!key) {
      flush();
      result.push(entry);
      continue;
    }

    if (bucketKey === key) {
      bucket.push(entry);
    } else {
      flush();
      bucket = [entry];
      bucketKey = key;
    }
  }
  flush();

  return result;
}

export function processGroupEntries(entry: TimelineEventEntry): TimelineEntry[] {
  const entries = (entry.processEvents ?? []).map((event): TimelineEventEntry => ({
    type: "event",
    id: `process:${entry.event.id}:${event.id}`,
    createdAt: event.createdAt,
    turnSeq: event.turnSeq,
    intraTurnOrder: event.intraTurnOrder,
    event,
    isProcessChild: true,
  }));
  return mergeAdjacentTimelineGroups(entries);
}
