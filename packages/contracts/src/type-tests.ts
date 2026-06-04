import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineEvent,
  AgentTimelineEventKind,
  AgentInteractionRequest,
  AgentInteractionResponse,
  ChatAttachment,
  ChatContextSearchResult,
  ChatMessage,
  TaskTodo,
  TimelineDisplayInput,
  ToolConsentRequest,
  ToolConsentResponsePayload,
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

export type ChatAttachmentSchemaTypeTest = Assert<
  Extends<
    {
      id: "att-1";
      name: "README.md";
      path: "D:/PROJECT/workspace/Lilia/README.md";
      kind: "file";
      size: 42;
      exists: true;
      mime: null;
      directory: null;
    },
    ChatAttachment
  >
>;

export type ChatContextSearchResultSchemaTypeTest = Assert<
  Extends<
    {
      attachment: {
        id: "att-1";
        name: "src";
        path: "D:/PROJECT/workspace/Lilia/src";
        kind: "directory";
        size: null;
        directory: {
          fileCount: 12;
          directoryCount: 3;
          totalSize: 2048;
          truncated: false;
          unreadableCount: 0;
        };
      };
      relativePath: "src";
      matchedBy: "name";
    },
    ChatContextSearchResult
  >
>;

export type ChatMessageIncludesAttachmentsTypeTest = Assert<
  Extends<
    {
      id: "u-1";
      taskId: "t-1";
      role: "user";
      content: "see attached";
      attachments: [];
      createdAt: 1;
    },
    ChatMessage
  >
>;

export type TaskTodoAllowsGuideAttachmentsTypeTest = Assert<
  Extends<
    {
      id: "todo-1";
      taskId: "task-1";
      text: "see file";
      done: false;
      order: 0;
      source: "lilia";
      priority: "normal";
      guideStatus: "pending";
      attachments: ChatAttachment[];
      createdAt: 1;
      updatedAt: 1;
    },
    TaskTodo
  >
>;

export type ToolConsentRequestTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      turnId: "turn-1";
      backend: "claude";
      requestId: "tool-1";
      toolName: "Bash";
      input: { command: "pwd" };
      title: null;
      displayName: null;
      description: null;
      blockedPath: null;
      decisionReason: null;
      toolUseId: null;
    },
    ToolConsentRequest
  >
>;

export type ToolConsentResponseUpdatedInputTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      requestId: "tool-1";
      decision: "allow";
      message: null;
      updatedInput: { command: "pwd && echo ok" };
    },
    ToolConsentResponsePayload
  >
>;

export type AgentInteractionAskRequestTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      turnId: "turn-1";
      backend: "codex";
      requestId: "ask-1";
      kind: "ask_user";
      payload: {
        title: "Codex 想确认一下";
        questions: [{
          id: "q-1";
          question: "选哪个方案？";
          mode: "single";
          options: [{ id: "o-1"; label: "A" }, { id: "o-2"; label: "B" }];
        }];
      };
    },
    AgentInteractionRequest
  >
>;

export type AgentInteractionToolRequestTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      turnId: "turn-1";
      backend: "codex";
      requestId: "tool-1";
      kind: "tool_consent";
      payload: {
        toolName: "commandExecution";
        input: { command: "yarn test" };
        title: "Run command";
        backend: "codex";
      };
    },
    AgentInteractionRequest
  >
>;

export type AgentInteractionToolResponseTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      requestId: "tool-1";
      kind: "tool_consent";
      result: {
        taskId: "task-1";
        requestId: "tool-1";
        decision: "deny";
        message: "先不执行";
      };
    },
    AgentInteractionResponse
  >
>;

