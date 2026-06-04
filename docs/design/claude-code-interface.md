# Claude Code 接口接入记录

> 状态：本文是 Lilia 对 Claude Code / Claude Agent SDK 接口的本地接入清单。
> 核对时间：2026-06-04。
> 核对版本：`@anthropic-ai/claude-agent-sdk 0.3.145`。

## 核对方式

本文只覆盖 Claude Code / Claude Agent SDK 层，不覆盖 Anthropic REST Messages API、Files API、Batch API 或 Admin API。

核对来源：

- Lilia Claude runner：`apps/desktop/agent-runner/claude/runClaude.mjs`、`apps/desktop/agent-runner/claude/permissions.mjs`、`apps/desktop/agent-runner/claude/timeline.mjs`。
- Lilia 扩展管理：`apps/desktop/src-tauri/src/plugins/*`、`apps/desktop/agent-runner/runtimeExtensions.mjs`。
- Lilia 工具归一化：`packages/contracts/src/claudeTools.mjs`。
- 本地 SDK 类型：`node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts`、`bridge.d.ts`、`assistant.d.ts`。
- 官方文档：[Agent SDK TypeScript](https://platform.claude.com/docs/en/agent-sdk/typescript)、[TypeScript v2 preview](https://platform.claude.com/docs/en/agent-sdk/typescript-v2-preview)、[Permissions](https://platform.claude.com/docs/en/agent-sdk/permissions)、[MCP](https://platform.claude.com/docs/en/agent-sdk/mcp)、[Plugins](https://platform.claude.com/docs/en/agent-sdk/plugins)。

判定口径：

- Lilia 只镜像 Claude 原生能力和事件，不伪造 Skills、Plugins、Hooks、Subagents 的协议语义。
- “事件已显示”不等于“管理能力已接入”。例如 Lilia 能显示 subagent 进度，但还没有 subagent 定义 / 调度管理。
- SDK 标注为 beta / alpha / experimental 的接口单独列出，默认按高变动面处理。

## 状态标记

| 状态 | 含义 |
|---|---|
| 已接入 | Lilia 已调用该接口 / 字段，并接到现有产品行为。 |
| 部分接入 | Lilia 已处理相邻能力或通用事件，但未完整覆盖该接口的字段语义、配置或管理面。 |
| 未接入 | Lilia 当前没有调用、配置或消费该接口。 |
| 实验性未接入 | SDK 标注 beta / alpha / experimental，且 Lilia 当前没有接入。 |

## 已接入接口

| 接口 / 能力 | Lilia 状态 | 说明 |
|---|---|---|
| `query()` 主入口 | 已接入 | Claude turn 通过 `query({ prompt, options })` 启动；prompt 由 `singleClaudePromptStream` 转成 SDK user message。 |
| `options.cwd` | 已接入 | 使用任务项目目录，缺省回退 runner 当前目录。 |
| `options.model` | 已接入 | 由 composer 传入；未传时使用 Claude CLI / SDK 默认。 |
| `options.resume` | 已接入 | Lilia 保存 SDK `session_id`，后续同 task + backend 自动 resume。 |
| `options.includePartialMessages` | 已接入 | 打开流式 partial message，用于 text / thinking timeline。 |
| `options.systemPrompt` | 已接入 | 使用 `{ type: "preset", preset: "claude_code" }`，Windows 下追加平台命令说明。 |
| `options.permissionMode` | 已接入 | Lilia `ask` / `readonly` 映射为 `default`；Plan 首轮映射为 `plan`；`full` 映射为 `bypassPermissions`。 |
| `options.allowDangerouslySkipPermissions` | 已接入 | 仅 `permission === "full"` 时传入，用于启用 `bypassPermissions`。 |
| `options.canUseTool` | 已接入 | 统一接入 Lilia 工具确认、Plan 确认、只读拒绝和 AskUser 内置工具放行。 |
| Claude Plan / `ExitPlanMode` | 已接入 | `ExitPlanMode` 镜像为 `kind: "plan"` timeline，并通过 AskUser 完成同意、取消和修订请求。 |
| `Query.setPermissionMode()` | 已接入 | Plan 确认后异步恢复执行阶段权限模式。 |
| SDK MCP server | 已接入 | 通过 `createSdkMcpServer` 注册内置 `lilia` server。 |
| SDK MCP tool | 已接入 | 通过 `tool()` 注册 `ask_user_question`，并设置 `alwaysLoad`。 |
| `options.mcpServers` stdio | 已接入 | 外部 Claude MCP server 由 Lilia 自管 JSON 注入；当前只支持 stdio。 |
| `options.toolAliases` | 已接入 | 将 `AskUserQuestion` 映射到 `mcp__lilia__ask_user_question`。 |
| `options.toolConfig.askUserQuestion.previewFormat` | 已接入 | 当前固定为 `markdown`。 |
| `options.skills` | 已接入 | 用户级和项目级 Skills 按启用列表传给 SDK。 |
| `options.plugins` | 已接入 | 用户级本地 Plugins 按 `{ type: "local", path }` 传给 SDK。 |
| `options.hooks` | 已接入 | 仅注册 `PostToolUse` / `PostToolUseFailure`，用于 Bash 命令被用户修改后的 additional context。 |
| `assistant` / `result` / `user` / `tool_result` | 已接入 | 映射为最终回复、turn 完成、工具结果等 timeline 事实事件。 |
| `stream_event` text / thinking | 已接入 | 按 content block 类型区分最终回复文本和 reasoning，不做字段猜测。 |
| 系统事件映射 | 已接入 | 已处理 tool progress / summary、auth status、subagent task、notification、api retry、status、session state、hook lifecycle、permission denied、mirror error。 |
| Claude 内置工具显示 | 已接入 | 已归一化 Bash、Read/Write/Edit/MultiEdit、LS/Glob/Grep、NotebookRead/Edit、WebSearch/WebFetch、TodoWrite、Task/Agent、ExitPlanMode。 |
| 未登记工具兜底 | 已接入 | 未登记 Claude 工具落到 Lilia 通用 `tool` kind，保留 toolName 和 input。 |

## 部分接入接口

| 接口 / 能力 | Lilia 状态 | 说明 |
|---|---|---|
| Sessions | 部分接入 | Lilia 保存 `session_id` 并 resume；未接入 `continue`、`forkSession`、`resumeSessionAt`、`sessionId` 或 session 管理 API。 |
| Subagents / `Task` / `Agent` | 部分接入 | 可显示 Task / Agent 调用、任务进度和通知；未提供 subagent 定义、管理或主动调度 UI。 |
| Plugins | 部分接入 | 可发现和启停用户级本地 plugin；未做安装、更新、项目级 / marketplace 作用域管理。 |
| Hooks | 部分接入 | 可注册少量 SDK hook，并能显示 hook lifecycle 事件；未提供 hooks 配置管理和结果面板。 |
| MCP | 部分接入 | 可管理 stdio server；未接入 HTTP、SSE、SDK instance MCP、tool policy、OAuth 或 elicitation 完整流程。 |
| Thinking / effort | 部分接入 | 可显示 thinking stream；未暴露 `thinking`、`effort`、`maxThinkingTokens` 配置。 |
| Permission events | 部分接入 | 可处理 `canUseTool` 和 permission denied 事件；未接入 `allowedTools` / `disallowedTools` / tool base set 的配置面。 |

## 未接入接口

| 接口 / 能力 | Lilia 状态 | 说明 |
|---|---|---|
| `options.allowedTools` | 未接入 | Lilia 当前通过 `canUseTool` 做运行时确认，不维护 SDK auto-allow 列表。 |
| `options.disallowedTools` | 未接入 | 未将禁用工具列表传给 SDK。 |
| `options.tools` | 未接入 | 未限制或替换 Claude Code 内置工具基集。 |
| `options.permissionPromptToolName` | 未接入 | 权限确认走 Lilia `canUseTool`，未改用 MCP permission prompt tool。 |
| `options.settings` | 未接入 | 未向 SDK 注入 flag settings。 |
| `options.managedSettings` | 未接入 | 未向 SDK 注入 managed / policy tier settings。 |
| `options.settingSources` | 未接入 | 未控制 user / project / local settings 加载来源。 |
| `options.sandbox` | 未接入 | 未启用 SDK sandbox 配置。 |
| `options.additionalDirectories` | 未接入 | 未通过 SDK 扩展可访问目录。 |
| `options.outputFormat` | 未接入 | 未使用 JSON schema structured output。 |
| `options.abortController` | 未接入 | Lilia 当前通过终止 runner 子进程打断 turn，而不是 SDK abort controller。 |
| `options.maxTurns` | 未接入 | 未设置 SDK turn 上限。 |
| `options.maxBudgetUsd` | 未接入 | 未设置 SDK 美元预算上限。 |
| `options.onElicitation` | 未接入 | MCP elicitation 未接入 Lilia 表单 / URL 授权流程。 |
| `options.includeHookEvents` | 未接入 | 当前未主动打开全量 hook lifecycle 输出；但 SDK 仍可能上报部分系统事件并被 timeline 兜底显示。 |
| `options.forwardSubagentText` | 未接入 | 未渲染完整 nested subagent transcript。 |
| `options.promptSuggestions` | 未接入 | 未显示 Claude 预测的下一条用户提示。 |
| `options.agentProgressSummaries` | 未接入 | 未启用 SDK 生成的 subagent 周期性进度摘要。 |
| `startup()` warm query | 未接入 | 每轮由 runner 子进程直接创建 query。 |
| `spawnClaudeCodeProcess` | 未接入 | 未自定义 Claude Code 进程启动方式。 |
| `pathToClaudeCodeExecutable` | 未接入 | 使用 SDK 默认可执行文件解析。 |
| `listSessions()` | 未接入 | 未从 Claude SDK 读取历史 session 列表。 |
| `getSessionMessages()` | 未接入 | 未从 Claude transcript 回补 timeline。 |
| `getSessionInfo()` | 未接入 | 未读取 Claude session 元信息。 |
| `forkSession()` | 未接入 | 未暴露 fork 历史会话。 |
| `listSubagents()` | 未接入 | 未读取 session 下 subagent 列表。 |
| `getSubagentMessages()` | 未接入 | 未读取 subagent transcript。 |
| `renameSession()` / `tagSession()` / `deleteSession()` | 未接入 | Lilia 使用自己的 task/session 元数据管理。 |

## 实验性 / Alpha 接口

| 接口 / 能力 | Lilia 状态 | 说明 |
|---|---|---|
| `options.betas` | 实验性未接入 | 本地 SDK 类型当前列出 `context-1m-2025-08-07`，Lilia 未传 beta header。 |
| `AgentDefinition.criticalSystemReminder_EXPERIMENTAL` | 实验性未接入 | 未通过 `agents` 定义自定义 subagent，也未使用该 experimental 字段。 |
| `options.sessionStore` | 实验性未接入 | SDK 标注 alpha；Lilia 未把 Claude transcript mirror 到外部 store。 |
| `options.sessionStoreFlush` | 实验性未接入 | 依赖 `sessionStore`，未接入。 |
| `options.loadTimeoutMs` | 实验性未接入 | 依赖 `sessionStore` resume materialization，未接入。 |
| `options.taskBudget` | 实验性未接入 | SDK 标注 alpha，且依赖 API beta header；Lilia 未传。 |
| `/bridge` remote control | 实验性未接入 | `attachBridgeSession`、`createCodeSession`、`fetchRemoteCredentials` 等为 alpha；Lilia 不接 Claude remote control。 |
| `/assistant` worker | 实验性未接入 | `runAssistantWorker` 和 assistant worker state 为 alpha；Lilia 当前使用本地 runner 子进程。 |

## 当前运行时边界

Claude turn 的 Lilia 数据流：

```text
Tauri chat runner
  -> node agent-runner.mjs
  -> @anthropic-ai/claude-agent-sdk query()
  -> Lilia protocol NDJSON
  -> Rust AgentRuntimeEvent
  -> agent timeline / AskUser / tool consent / todo mirror
```

关键边界：

- Lilia 不直接改写 Claude transcript；只保存自己的 task timeline、composer 状态和 SDK session id。
- Claude 原生 Todo、Task、Plan 只做镜像和展示；Lilia 不把它们改造成自有协议后再喂回 Claude。
- 只读模式是 Lilia 运行时门禁：读工具白名单之外的工具会被拒绝并写入 timeline。
- 外部 MCP server 名称 `lilia` 保留给内置 AskUser server，用户配置中同名 server 会被跳过。
- Plugins 目前按本地 path 传给 SDK；Lilia 的 `disabled: true` 是自管字段，不是 Claude 官方字段。

## 接入优先级

1. **MCP 能力补齐**：HTTP / SSE transport、tool policy、elicitation 和 auth 流程需要先设计 UI 与安全边界。
2. **Subagents 管理**：读取 / 定义 `agents`，并把 Task / Agent 事件与可调度列表打通。
3. **Hooks 管理**：展示 hooks 配置、执行结果和失败诊断，而不是只消费生命周期事件。
4. **Session 恢复增强**：评估 `listSessions` / `getSessionMessages` / `forkSession` 是否用于导入或回补 Claude 历史。
5. **SDK 设置面**：逐步暴露 `allowedTools`、`disallowedTools`、`settings`、`sandbox`、`thinking`、`effort` 等高级选项。
6. **Alpha / beta 接口**：除非有明确产品目标，`sessionStore`、remote control、assistant worker 和 task budget 继续按高变动面观察。

## 升级复核清单

升级 `@anthropic-ai/claude-agent-sdk` 后，按以下顺序重新核对：

1. 查看 `sdk.d.ts` 中 `Options`、`PermissionMode`、`HookEvent`、`SdkBeta`、`McpServerConfig`、`SDKMessage`、`SdkPluginConfig` 是否新增或变更。
2. 查看 `bridge.d.ts` 和 `assistant.d.ts` 的 alpha 接口是否升级为稳定接口或发生破坏性变更。
3. 对照 `runClaude.mjs` 的 `options` 构造，确认新增字段是否需要显式默认值或产品开关。
4. 对照 `timeline.mjs` 和 `claudeTools.mjs`，确认新增 SDK message / tool 是否需要专门归一化。
5. 复查官方文档链接和本文核对时间，避免把旧 SDK 的能力误判为当前可用接口。
