import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineEvent,
  AgentTimelineEventKind,
  TimelineDisplayInput,
} from "./index";
import { deriveTimelineDisplay } from "./index";

type Assert<T extends true> = T;
type Extends<From, To> = From extends To ? true : false;

type DetailCases =
  | { type: "line"; text: "done"; tone: "muted" }
  | { type: "fields"; fields: [{ label: "cwd"; value: "C:/repo" }] }
  | { type: "code"; label: "OUTPUT"; content: "ok"; language: "text" }
  | { type: "markdown"; content: "**ok**"; singleLine: false }
  | { type: "list"; items: [{ text: "one"; tone: "success" }]; ordered: true };

export type AgentTimelineOpenKindTypeTest = Assert<
  Extends<"extension_index", AgentTimelineEventKind>
>;

export type AgentTimelineEventHasNoDisplayTypeTest = Assert<
  Extends<"display" extends keyof AgentTimelineEvent ? true : false, false>
>;

export type DeriveTimelineDisplayReturnsDisplayTypeTest = Assert<
  Extends<ReturnType<typeof deriveTimelineDisplay>, AgentTimelineDisplay>
>;

export type TimelineDisplayInputShapeTypeTest = Assert<
  Extends<
    {
      kind: "command";
      status: "success";
      title: "Bash";
      summary: null;
      payload: { command: "ls" };
    },
    TimelineDisplayInput
  >
>;

export type AgentTimelineDisplayDetailSchemaTypeTest = Assert<
  Extends<DetailCases, AgentTimelineDisplayDetail>
>;

