import { describe, expect, it } from "vitest";
import type { AgentTimelineEvent } from "@lilia/contracts";
import { taskTimelineLoadMergePlan } from "../src/pages/taskDetail/taskTimelineLoadMerge";
import { timelineEventFixture } from "./timelineTestHelpers";

function event(overrides: Partial<AgentTimelineEvent>): AgentTimelineEvent {
  return timelineEventFixture(overrides, {
    status: "running",
    title: "命令",
  });
}

describe("taskTimelineLoadMergePlan", () => {
  it("keeps new current events that appeared while loading", () => {
    const plan = taskTimelineLoadMergePlan({
      loadedEvents: [event({ id: "loaded" })],
      currentEvents: [
        event({ id: "old-current" }),
        event({ id: "new-current" }),
      ],
      eventIdsBeforeLoad: new Set(["old-current"]),
      liveEventsDuringLoad: new Map(),
    });

    expect([...plan.preserveEventIds]).toEqual(["new-current"]);
    expect(plan.liveEventsToReplay).toEqual([]);
  });

  it("preserves and replays live updates that are newer than the loaded snapshot", () => {
    const live = event({
      id: "same-event",
      status: "requires_action",
      summary: "live",
      updatedAt: 30,
    });
    const plan = taskTimelineLoadMergePlan({
      loadedEvents: [event({ id: "same-event", status: "running", summary: "loaded", updatedAt: 20 })],
      currentEvents: [live],
      eventIdsBeforeLoad: new Set(["same-event"]),
      liveEventsDuringLoad: new Map([[live.id, live]]),
    });

    expect([...plan.preserveEventIds]).toEqual(["same-event"]);
    expect(plan.liveEventsToReplay).toEqual([live]);
  });

  it("lets a newer loaded snapshot replace stale live updates", () => {
    const staleLive = event({
      id: "same-event",
      status: "requires_action",
      summary: "stale live",
      updatedAt: 20,
    });
    const plan = taskTimelineLoadMergePlan({
      loadedEvents: [event({ id: "same-event", status: "success", summary: "loaded", updatedAt: 30 })],
      currentEvents: [staleLive],
      eventIdsBeforeLoad: new Set(["same-event"]),
      liveEventsDuringLoad: new Map([[staleLive.id, staleLive]]),
    });

    expect([...plan.preserveEventIds]).toEqual([]);
    expect(plan.liveEventsToReplay).toEqual([]);
  });
});
