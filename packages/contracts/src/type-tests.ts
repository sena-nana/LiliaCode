import type {
  AgentTimelineDisplay,
  AgentTimelineDisplayDetail,
  AgentTimelineEvent,
  AgentTimelineEventKind,
  AgentInteractionRequest,
  AgentInteractionResponse,
  AgentInteractionSettings,
  ChatAttachment,
  ChatContextSearchResult,
  ChatMessage,
  ChatRuntimeCommand,
  ChatSlashCommandSearchResult,
  ChatWorkflow,
  LiliaIabSnapshot,
  LiliaIabSubmitResult,
  LiliaReviewTarget,
  CodexProfileSettings,
  ProjectRoadmap,
  SuggestionItem,
  TaskTodo,
  TimelineDisplayInput,
  ToolConsentRequest,
  ToolConsentResponsePayload,
  ProjectArchitectureGraph,
  ProjectArchitectureChange,
  ProjectArchitectureInteractionPayload,
  ProviderRuntimeOptions,
  CodexAccountQuotaStatus,
  QuotaUsageStats,
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

export type LiliaIabSnapshotTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      url: "https://example.com/";
      title: "Example";
      note: "按钮没有响应";
      capturedAt: 1;
      screenshotPath: "C:/Users/me/.lilia/cache/iab-snapshots/iab.png";
      screenshotAttachment: ChatAttachment;
      status: "captured";
      warning: null;
    },
    LiliaIabSnapshot
  >
>;

export type LiliaIabSubmitResultTypeTest = Assert<
  Extends<
    {
      snapshot: LiliaIabSnapshot;
      delivery: "runner";
      stdinForwarded: true;
    },
    LiliaIabSubmitResult
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

export type LiliaFixSuggestionWorkflowTypeTest = Assert<
  Extends<
    {
      type: "lilia_fix_suggestion";
      target: { type: "baseBranch"; branch: "main" };
      instructions: "只给建议";
      mode: "suggest";
    },
    ChatWorkflow
  >
>;

export type LiliaBatchApplyWorkflowTypeTest = Assert<
  Extends<
    {
      type: "lilia_batch_apply";
      sourceTurnId: "turn-1";
      sourceKind: "fix_suggestion";
      sourceSummary: "建议修复权限边界";
      instructions: "应用最小改动";
    },
    ChatWorkflow
  >
>;

export type LiliaSessionForkRuntimeCommandTypeTest = Assert<
  Extends<
    {
      type: "lilia_session_fork";
      excludeTurns: true;
    },
    ChatRuntimeCommand
  >
>;

export type LiliaSessionForkIsNotWorkflowTypeTest = Assert<
  Extends<
    {
      type: "lilia_session_fork";
      excludeTurns: true;
    },
    ChatWorkflow
  > extends true ? false : true
>;

export type LiliaSessionManagementRuntimeCommandTypeTest = Assert<
  Extends<
    {
      type: "lilia_session_management";
      action: "tag";
      sessionId: "thread-1";
      tag: "release";
      archived: true;
      limit: 20;
      cursor: "cursor-1";
      searchTerm: "bug";
      includeSystemMessages: true;
    },
    ChatRuntimeCommand
  >
>;

export type LiliaSessionManagementIsNotWorkflowTypeTest = Assert<
  Extends<
    {
      type: "lilia_session_management";
      action: "list";
    },
    ChatWorkflow
  > extends true ? false : true
>;

export type LiliaProviderSettingsRuntimeCommandTypeTest = Assert<
  Extends<
    {
      type: "lilia_provider_settings";
      action: "update";
    },
    ChatRuntimeCommand
  >
>;

export type LiliaProviderSettingsRuntimeCommandRejectsInlineOptionsTypeTest = Assert<
  Extends<
    {
      type: "lilia_provider_settings";
      action: "update";
      common: { model: "gpt-5.5"; permission: "ask" };
      runtimeOptions: ProviderRuntimeOptions;
    },
    ChatRuntimeCommand
  > extends true ? false : true
>;

export type LiliaProviderSettingsIsNotWorkflowTypeTest = Assert<
  Extends<
    {
      type: "lilia_provider_settings";
      action: "update";
    },
    ChatWorkflow
  > extends true ? false : true
>;

export type ProviderRuntimeOptionsTypeTest = Assert<
  Extends<
    {
      common: {
        model: "gpt-5.5";
        permission: "ask";
        runtimeWorkspaceRoots: ["D:/PROJECT/workspace/Lilia"];
      };
      provider: {
        codex: {
          profile: "deep";
          reasoningEffort: "high";
          runtimeWorkspaceRoots: ["D:/PROJECT/workspace/Lilia"];
          persistExtendedHistory: true;
          environments: [{ id: "env-1" }];
          experimentalRawEvents: true;
          responsesApiClientMetadata: { surface: "lilia" };
        };
        claude: {
          allowedTools: ["Read"];
          disallowedTools: ["Bash"];
          additionalDirectories: ["D:/PROJECT/workspace/Lilia/docs"];
          maxTurns: 8;
          maxBudgetUsd: 1.5;
          tools: { type: "preset"; preset: "claude_code" };
          permissionPromptToolName: "mcp__lilia__permission_prompt";
          settings: { model: "claude-opus-4-5" };
          managedSettings: { sandbox: { enabled: true } };
          settingSources: ["user", "project"];
          sandbox: { enabled: true };
          outputFormat: { type: "json" };
          includeHookEvents: true;
          forwardSubagentText: true;
          agentProgressSummaries: true;
          continue: true;
          resumeSessionAt: "message-uuid";
          sessionId: "00000000-0000-4000-8000-000000000001";
          abortAfterMs: 3000;
          sessionStore: { explicit: true };
        };
      };
      experimentalProviderOptions: [{
        provider: "codex";
        capability: "raw-events";
        payload: { enabled: true };
        fallback: "diagnostic";
      }];
    },
    ProviderRuntimeOptions
  >
>;

export type QuotaUsageStatsTypeTest = Assert<
  Extends<
    {
      days: 7;
      backend: "all";
      rangeStart: 1;
      rangeEnd: 2;
      totals: {
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
      };
      cost: {
        knownCostUsd: 0.12;
        costRecordCount: 1;
        totalRecordCount: 2;
      };
      daily: [{
        dayStart: 1;
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
        knownCostUsd: 0.12;
        costRecordCount: 1;
        recordCount: 2;
      }];
      backends: [{
        backend: "claude";
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
        knownCostUsd: 0.12;
        costRecordCount: 1;
        recordCount: 1;
      }];
      recent: [{
        eventId: "event-1";
        taskId: "task-1";
        turnId: "turn-1";
        backend: "claude";
        sessionId: "session-1";
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
        knownCostUsd: 0.12;
        createdAt: 1;
      }];
      projects: [{
        projectId: "project-1";
        projectName: "Lilia";
        projectCwd: "C:/repo";
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
        knownCostUsd: 0.12;
        costRecordCount: 1;
        recordCount: 1;
      }];
      conversations: [{
        taskId: "task-1";
        taskTitle: "额度统计";
        taskStatus: "running";
        projectId: "project-1";
        projectName: "Lilia";
        inputTokens: 100;
        outputTokens: 50;
        cacheReadTokens: 10;
        cacheCreationTokens: 5;
        totalTokens: 165;
        knownCostUsd: 0.12;
        costRecordCount: 1;
        recordCount: 1;
      }];
      tools: [{
        key: "command::";
        label: "命令";
        kind: "command";
        subkind: null;
        toolName: null;
        callCount: 1;
        sharePercent: 100;
      }];
    },
    QuotaUsageStats
  >
>;

export type CodexAccountQuotaStatusTypeTest = Assert<
  Extends<
    {
      available: true;
      connectionMode: "codex-account";
      limitId: "codex";
      limitName: "Codex";
      planType: "pro";
      rateLimitReachedType: null;
      fiveHour: {
        usedPercent: 25;
        windowDurationMins: 300;
        resetsAt: 1;
      };
      weekly: {
        usedPercent: 40;
        windowDurationMins: 10080;
        resetsAt: 2;
      };
      fetchedAt: 3;
      error: null;
    },
    CodexAccountQuotaStatus
  >
>;

export type AutomationRunWorkflowTypeTest = Assert<
  Extends<
    {
      type: "automation";
      automationRunId: "run-1";
    },
    ChatWorkflow
  >
>;

export type SlashCommandSearchResultTypeTest = Assert<
  Extends<
    {
      command: {
        id: "project:release";
        name: "release";
        title: "生成发布检查";
        description: "按项目模板生成发布检查。";
        source: "project";
        parameters: [];
      };
      matchedBy: "name";
    },
    ChatSlashCommandSearchResult
  >
>;

export type SlashCommandWorkflowTypeTest = Assert<
  Extends<
    {
      type: "slash_command";
      commandId: "native:help";
      source: "native";
      arguments: {};
    },
    ChatWorkflow
  >
>;

export type LiliaFixSuggestionTargetTypeTest = Assert<
  Extends<{ type: "commit"; sha: "abc123" }, LiliaReviewTarget>
>;

export type LiliaFixSuggestionRejectsInvalidTargetTypeTest = Assert<
  Extends<
    Extends<
      {
        type: "lilia_fix_suggestion";
        target: { type: "baseBranch" };
      },
      ChatWorkflow
    >,
    false
  >
>;

export type SuggestionItemIncludesGitHubActivityTypeTest = Assert<
  Extends<
    {
      id: "sg-1";
      projectId: "project-1";
      taskIds: [];
      source: "github";
      githubActivities: [{
        id: "gh-1";
        repoFullName: "sena-nana/LiliaCode";
        kind: "pull_request";
        title: "PR #1: 接入 GitHub 活动";
        url: "https://github.com/sena-nana/LiliaCode/pull/1";
      }];
      summary: "跟进 PR";
      reason: "近期 PR 有新活动。";
      prompt: "请继续跟进 PR #1。";
      generatedAt: 1;
    },
    SuggestionItem
  >
>;

export type SuggestionItemIncludesClaudeNativeSourceTypeTest = Assert<
  Extends<
    {
      id: "claude-suggestion-1";
      projectId: "project-1";
      taskIds: ["task-1"];
      source: "claude";
      githubActivities: [];
      summary: "继续检查建议展示";
      reason: "Claude 根据上一轮对话预测的下一条提示。";
      prompt: "请继续检查 Claude 原生建议展示。";
      generatedAt: 1;
    },
    SuggestionItem
  >
>;

export type CodexProfileSettingsShapeTypeTest = Assert<
  Extends<
    {
      profile: "balanced";
      model: "gpt-5.5";
      reasoningEffort: "high";
      runtimeWorkspaceRoots: ["C:/repo", "D:/shared"];
      responsesApiClientMetadata: { surface: "lilia" };
      additionalContext: "本轮额外上下文";
      persistExtendedHistory: true;
      initialTurnsPage: { limit: 20 };
      excludeTurns: ["turn-old"];
    },
    CodexProfileSettings
  >
>;

export type AgentInteractionSettingsIncludesCodexProfileTypeTest = Assert<
  Extends<
    {
      nonInterruptMode: false;
      debug: true;
      agentRuntimeChannel: "mutsuki_core";
      codexProfile: CodexProfileSettings;
    },
    AgentInteractionSettings
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

export type ProjectRoadmapSnapshotTypeTest = Assert<
  Extends<
    {
      milestones: [{
        id: "m-1";
        projectId: "p-1";
        title: "M1";
        description: "";
        status: "upcoming";
        dueDate: null;
        order: 0;
        createdAt: 1;
      }];
      links: [{ taskId: "t-1"; milestoneId: "m-1" }];
    },
    ProjectRoadmap
  >
>;

export type ProjectArchitectureGraphTypeTest = Assert<
  Extends<
    {
      projectId: "p-1";
      version: 1;
      summary: "桌面应用";
      nodes: [{
        id: "desktop-ui";
        label: "Desktop UI";
        type: "component";
        summary: "Vue 前端";
        paths: ["apps/desktop/src"];
        tags: ["frontend"];
      }];
      edges: [{
        id: "ui-tauri";
        from: "desktop-ui";
        to: "tauri";
        type: "calls";
        label: "IPC";
        summary: "通过 invoke 调用 Tauri 命令";
      }];
      updatedAt: 1;
    },
    ProjectArchitectureGraph
  >
>;

export type ProjectArchitectureChangeTypeTest = Assert<
  Extends<
    | {
        type: "upsert_node";
        node: ProjectArchitectureGraph["nodes"][number];
      }
    | {
        type: "set_summary";
        summary: "跨端契约优先";
      },
    ProjectArchitectureChange
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
      additionalPermissions: [{ permission: "network" }];
      availableDecisions: ["accept", "decline"];
      proposedExecpolicyAmendment: { sandbox: "workspace-write" };
      proposedNetworkPolicyAmendments: [{ host: "example.com" }];
      networkApprovalContext: { host: "example.com" };
      cwd: "D:/PROJECT/workspace/Lilia";
      reason: "needs tests";
      commandActions: [{ text: "pwd" }];
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
      codexDecision: "accept";
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
        additionalPermissions: [{ permission: "network" }];
        availableDecisions: ["accept", "decline", "cancel"];
        proposedExecpolicyAmendment: { sandbox: "workspace-write" };
        proposedNetworkPolicyAmendments: [{ host: "registry.npmjs.org" }];
        networkApprovalContext: { host: "registry.npmjs.org" };
        cwd: "D:/PROJECT/workspace/Lilia";
        reason: "run tests";
        commandActions: [{ text: "yarn test" }];
      };
    },
    AgentInteractionRequest
  >
>;

export type AgentInteractionPlanApprovalRequestTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      turnId: "turn-1";
      backend: "codex";
      requestId: "plan-1";
      kind: "plan_approval";
      payload: {
        title: "确认 Codex 计划";
        source: "Codex Plan";
        intent: "plan_approval";
        questions: [{
          id: "approve-plan";
          question: "";
          mode: "confirm";
          confirmLabel: "按计划执行";
        }];
      };
    },
    AgentInteractionRequest
  >
>;

export type AgentInteractionArchitectureRequestTypeTest = Assert<
  Extends<
    {
      taskId: "task-1";
      turnId: "turn-1";
      backend: "claude";
      requestId: "arch-1";
      kind: "architecture_change";
      payload: ProjectArchitectureInteractionPayload;
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
        codexDecision: "decline";
      };
    },
    AgentInteractionResponse
  >
>;
