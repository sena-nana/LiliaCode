import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineEvent,
  AgentTimelineEventKind,
} from "./index";

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

export type AgentTimelineDisplayRequiredTypeTest = Assert<
  Extends<AgentTimelineEvent["display"], AgentTimelineDisplay>
>;

export type AgentTimelineDisplayDetailSchemaTypeTest = Assert<
  Extends<DetailCases, AgentTimelineDisplayDetail>
>;
