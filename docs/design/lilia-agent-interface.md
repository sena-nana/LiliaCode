# Lilia 统一接口对接文档

> 状态：本文是 Lilia 应用层对 Claude / Codex provider adapter 的统一接口状态文档。
> 核对时间：2026-06-13。
> 迁移来源：已删除的 `docs/design/claude-code-interface.md`、`docs/design/codex-experimental-app-server.md`，历史内容以 Git 历史为准。

## 核对口径

应用层只面向 Lilia 接口，不直接区分 Claude 接口或 Codex 接口。Claude / Codex 只作为 provider adapter 消费同一组 Lilia workflow、interaction、history、extension 和 timeline 契约。

本文覆盖：

- `packages/contracts/src/chat.ts` 中的 `ChatWorkflow` 和运行时输入契约。
- `packages/contracts/src/agent-interaction.ts` 中的 `AgentInteractionKind`。
- `packages/contracts/src/codex-history.ts`、`packages/contracts/src/claude-history.ts` 中的历史导入契约。
- `packages/contracts/src/plugins.ts` 中的 `ClaudeRuntimeExtensions` / `CodexRuntimeExtensions` 运行时扩展契约。
- `apps/desktop/agent-runner/codex/runCodex.mjs`、`apps/desktop/agent-runner/claude/runClaude.mjs` 中的 provider adapter 实现。

状态含义：

| 状态 | 含义 |
|---|---|
| 已接入 | provider 有原生接口或 Lilia 替代方案，且已接入产品行为。 |
| 部分接入 | Lilia 已覆盖主流程，但 provider 的完整字段、管理面或高级能力未完全接入。 |
| 诊断替代 | provider 没有等价原生接口，Lilia 写入 diagnostic timeline 作为明确替代实现。 |
| 预设落点 | 已有 Lilia 协议落点名称和双 provider 语义；当前产品主流程未启用时返回 diagnostic / unsupported result。 |

## Lilia 落点语义契约

后续实现先查本节确认 Lilia 落点的产品含义，再查“Lilia 协议落点索引”确认 Codex / Claude adapter 的实现方式。禁止以 provider 方法名作为应用层协议名；新增 provider 能力必须先落到本节已有落点，或先补充新的 Lilia 落点语义。

通用规则：

- 输入形状以 `packages/contracts` 为准；provider adapter 不得要求 UI 传 provider 专属 payload。
- 成功输出统一回到 Lilia task、timeline、runtime snapshot、history result、plugin result 或 interaction response。
- provider 不支持某能力时，adapter 必须返回 Lilia diagnostic / empty result / unsupported result 中的一种明确结果，不能静默失败。
- provider 原生状态只能作为实现细节；应用层持久状态以 Lilia task、session checkpoint、timeline 和 settings 为准。

### 当前落点语义

| Lilia 落点 | 含义 | 输入 | 输出 / 副作用 | 失败 / 降级语义 |
|---|---|---|---|---|
| `chat_send_message` 普通 turn | 向当前 task 的当前 provider 发送一轮 agent 输入。 | `ChatComposerState`、content、attachments、可选 `ChatWorkflow`、project cwd。 | 创建或排队 Lilia turn，写用户消息和 agent timeline，完成后更新 session checkpoint。 | provider 启动或执行失败时写 `error` timeline；不改写用户草稿外的状态。 |
| `ChatComposerState.model` | 本轮或后续 turns 的模型选择。 | `model: string`。 | 作为 runner payload 进入 provider adapter。 | provider 不支持指定模型时由 provider 返回错误并写 Lilia timeline。 |
| `PermissionMode` | Lilia 对工具、文件、命令权限的统一模式。 | `full`、`ask`、`readonly`。 | adapter 映射到 provider 权限配置，并把需要确认的操作转为 `tool_consent` / `permission_approval`。 | 无 provider 等价项时由 Lilia runtime 拦截或降级为 tool consent。 |
| `planMode` | 请求 agent 先产出计划并等待用户确认。 | boolean。 | 产生 `plan` timeline，并通过 `plan_approval` 等待确认、取消或修订。 | provider 无原生 plan 时必须用 Lilia prompt / timeline 替代，不得直接执行。 |
| `conversationContext` / attachment context | 给 agent 提供当前任务、相关任务、附件和项目上下文。 | Lilia runner payload 中的 conversation context 与 attachments。 | provider 可读取上下文；timeline 中保留用户引用的附件。 | provider 无工具读取能力时，将上下文作为 prompt / additional context 注入。 |
| runtime session checkpoint | 记录 task 与 provider 原生会话的映射。 | task id、backend、runtime channel、provider session id。 | 后续 turn 自动 resume。 | checkpoint 缺失时创建新 provider session；不阻塞 Lilia task。 |
| runtime waiting state | 表示 agent 正等待用户在 Lilia UI 里响应。 | pending interaction id、kind、payload。 | UI 显示 pending action；响应写回 runner stdin。 | provider 无等待计数时只维护 Lilia pending state。 |
| `AgentTimelineEvent` | Lilia 统一事实流，承载消息、计划、工具、诊断、错误、goal 等。 | provider event 或 Lilia runtime event。 | 持久化并驱动 timeline UI。 | 未识别事件必须降级为 diagnostic 或通用 tool，不丢失原始摘要。 |
| `AgentTimelineEvent(kind=tool)` 兜底 | 展示未登记或未知 provider 工具调用。 | tool name、input、status、summary。 | 以通用工具卡展示。 | 无法解析字段时保留最小 tool name / input。 |
| interrupt / queue / rollback | Lilia 对 turn 生命周期的控制。 | task id、turn id、rollback payload。 | 中断、排队、回滚草稿或 timeline。 | provider 只接收当前 turn；Lilia runtime 负责恢复一致性。 |
| `lilia_review` | 对指定代码范围做审查。 | `LiliaReviewTarget`、instructions、delivery。 | 产生 review timeline / assistant 回复；detached 时可由 provider 支持独立交付。 | 无原生 review 时构造结构化 prompt。 |
| `lilia_fix_suggestion` | 针对审查目标生成修复建议或直接应用。 | `LiliaReviewTarget`、instructions、`mode`。 | `suggest` 产出建议；`apply` 进入执行和确认流程。 | 无原生能力时构造 Lilia prompt，仍必须遵守权限和 tool consent。 |
| `lilia_batch_apply` | 将 review / fix suggestion 的结果批量应用。 | source turn id、source kind、summary、instructions。 | 进入 Plan / apply 流程并写 batch apply timeline。 | 缺少 source 信息时拒绝启动并写错误。 |
| `lilia_goal` | 设置、刷新或清除当前线程目标。 | action、objective、status、tokenBudget。 | 更新 Lilia goal timeline / 状态行。 | provider 无原生 goal 时由 Lilia timeline 作为权威状态。 |
| `lilia_compact` | 压缩当前 provider 会话上下文。 | 当前 task session checkpoint。 | 写 compact start / completed / failed timeline。 | 无 session 或 provider 失败时写 diagnostic / error。 |
| `lilia_background_terminals_clean` | 清理当前会话相关后台终端。 | 当前 session id 或 thread id。 | 写清理结果 diagnostic。 | provider 无能力时写 unsupported diagnostic。 |
| `lilia_memory_mode` | 启用或关闭 provider 记忆模式。 | `enabled` 或 `disabled`。 | 写 memory mode 状态 diagnostic。 | provider 无能力时写 unsupported diagnostic，不模拟外部记忆。 |
| `lilia_memory_reset` | 重置 provider 记忆。 | 无额外输入。 | 写 reset 结果 diagnostic。 | provider 无能力时写 unsupported diagnostic，不删除 Lilia task history。 |
| `lilia_session_fork` | 从当前 provider session 分叉新 session。 | `excludeTurns?`。 | 新 session id 写入 `done.sessionId` 并成为后续 checkpoint。 | provider 无 session 时拒绝并写 error。 |
| `lilia_config_diagnostics` | 读取 provider 配置和要求的诊断摘要。 | `includeLayers?`。 | 写 diagnostic timeline；不修改配置。 | provider 无配置面时写 unsupported diagnostic。 |
| `lilia_provider_settings` | 诊断或更新高级 provider 设置。 | action、common、codex、claude。 | Codex 支持模型、权限、profile、reasoning effort、permission profile、workspace roots、extended history、受控 environments / raw events / metadata 透传；Claude 将 allowed / denied tools、additional directories、tool / settings / sandbox / output / hook / subagent / session resume 字段和 turn / budget / abort limit 接入 SDK query options。 | `update` 无有效字段时拒绝；高变动字段只在 workflow 显式传入时受控透传。 |
| `automation` | 自动化触发 agent turn。 | automation run id。 | 复用普通 turn 生命周期并回写自动化结果。 | agent 失败时自动化 run 标记失败。 |
| `slash_command` | 执行 Lilia native / project slash command。 | command id、source、arguments。 | 转为 Lilia workflow 或普通 turn。 | 未知命令拒绝并保留 composer 状态。 |
| `ask_user` | agent 向用户提问。 | `AskUserSpec`。 | UI 收集 `AskUserResult` 并写回 runner。 | 用户取消时把 cancelled result 返回 provider。 |
| `plan_approval` | 用户审批 agent 计划。 | `AskUserSpec(intent=plan_approval)`。 | 同意则进入执行；修订则继续 plan；取消则结束。 | provider 不得在同意前执行计划。 |
| `tool_consent` | 用户审批工具、命令或文件变更。 | `ToolConsentInteractionPayload`。 | 返回 allow / deny、可选更新后的 input、可选 provider decision。 | 拒绝时 provider 必须收到明确拒绝结果。 |
| edited command result | 用户编辑 provider 提议命令后由 Lilia 执行并回灌结果。 | 原命令、更新后命令、cwd、权限 profile。 | 执行输出摘要回灌给 provider 并写 timeline。 | 执行失败仍回灌失败摘要，不能假装原命令已执行。 |
| `permission_approval` | 审批 provider 提出的权限扩展。 | permissions、scope、reason。 | 返回批准后的 permissions / scope。 | provider 无等价能力时必须走 `PermissionMode` / `tool_consent`。 |
| `mcp_elicitation` | MCP server 请求用户填写表单或打开 URL。 | server name、mode、schema / url、message。 | 返回 accept / decline / cancel。 | 未接入 provider 时返回 unsupported diagnostic。 |
| `architecture_change` | agent 请求修改项目架构图。 | `ProjectArchitectureInteractionPayload`。 | 用户确认后写入 Lilia architecture 数据。 | 用户拒绝时 provider 收到拒绝结果。 |
| `HistoryImport*` search | 搜索可导入 provider 历史。 | provider、search term、cursor、limit、archived。 | 返回 `HistoryImportItem` 列表。 | provider 无原生 search 时使用 Lilia 本地解析或空结果。 |
| `HistoryImport*` preview | 预览 provider 历史并映射为 Lilia timeline。 | provider、item id、detail。 | 返回 messages、events、event count。 | 解析失败时返回错误，不创建 task。 |
| `HistoryImport*` attach | 将 provider 历史绑定到 Lilia task。 | provider、item id、mode、task / project id。 | 创建或更新 task、写 attach event、后台同步 timeline。 | attach 失败不得写半成品 session checkpoint。 |
| runtime state list | 查询 provider session 对应的 Lilia runtime 状态。 | 无或 project / backend filter。 | 返回 running / queued / pending 等 Lilia runtime state。 | provider 无独立状态时使用 Lilia runtime snapshot。 |
| task archive sync | Lilia task 归档后同步 provider 历史归档。 | task id / provider session id。 | best-effort provider sync。 | 同步失败只记录日志或 diagnostic，不阻塞 Lilia 归档。 |
| task title sync | Lilia task 标题更新后同步 provider session 标题。 | task id、title。 | best-effort provider sync 和 diagnostic。 | 同步失败不回滚 Lilia 标题。 |
| `AgentRuntimeExtensions` | 当前 turn 可用的运行时扩展集合。 | provider-specific extension config。 | 注入 adapter runtime。 | 单个扩展无效时进入 warnings，不阻塞其他扩展。 |
| `PluginsOverview` | 插件页统一概览。 | project cwd。 | 返回 `PluginSkill`、`PluginPackage`、`PluginMcpServer`、配置路径、warnings。 | provider 专属字段无内容时返回空集合。 |
| `PluginMcpServer` / `PluginMcpServerInput` | 管理 MCP server 配置。 | backend、name、command、args、env、enabled。 | 按 backend 更新 Claude / Codex config 并进入 runtime extensions。 | provider 不消费另一端配置；不混用配置源。 |
| `PluginSkill` | 管理 provider skill 能力。 | backend、scope、name、description、enabled、path。 | Claude Skills 启用后进入 Claude runtime extensions。 | Codex 当前返回空集合 / unsupported。 |
| `PluginPackage` | 管理 provider plugin/package 能力。 | backend、scope、name、version、enabled、path。 | Claude local Plugins 启用后进入 Claude runtime extensions。 | Codex 当前返回空集合 / unsupported。 |

## Lilia 协议落点索引

每一行都以 Lilia 落点为唯一应用层协议。Codex / Claude 只能作为 adapter 实现该 Lilia 落点；没有原生能力时，必须通过 Lilia 的诊断、空能力、task 元数据或统一 timeline 替代，不能新增绕过 Lilia 的 provider 专属应用入口。

表格中的 provider 列必须同时回答两件事：原生接口本来的含义是什么，以及它如何映射到 Lilia 落点。后续实现 adapter 时，不应只按 provider 方法名猜测语义。

### 对话与运行时

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `chat_send_message` 普通 turn | 在当前 Lilia task 中启动一轮用户到 agent 的输入，统一负责 session 创建、resume、流式 timeline 和 done / error 回写。 | 原生语义：`initialize` 建立 app-server RPC 能力协商，`thread/start` 创建新 thread，`thread/resume` 重新打开既有 thread，`turn/start` 在 thread 中追加一轮输入并开始执行。映射：Lilia 初始化时声明 `capabilities.experimentalApi = true`，按 checkpoint 选择 start / resume，再用 `turn/start` 承载本轮 prompt、cwd、权限、上下文和 workflow 派生输入。 | 原生语义：Claude SDK 没有单独初始化 RPC；`query({ prompt, options })` 创建或恢复一次 SDK query 流，prompt 是本轮用户输入，options 控制 cwd、model、resume、system prompt、权限和流式事件。映射：Lilia 每轮构造 `query()` options，开启 `options.includePartialMessages`，使用 Claude Code preset `options.systemPrompt`，并把 SDK message stream 归一为 Lilia timeline。 | 已接入 |
| `ChatComposerState.model` | 表示用户在 Lilia composer 或配置中选择的模型，作为当前 turn 或后续 turns 的模型意图。 | 原生语义：Codex `turn/start` 可携带本轮模型 / collaboration settings，`thread/settings/update` 可更新 thread 后续 turns 的 sticky model 等设置。映射：本轮模型优先进入 `turn/start` 或 plan collaboration settings；需要持久化时进入 `thread/settings/update`。 | 原生语义：`options.model` 指定 Claude SDK 本次 query 使用的模型，缺省时走 Claude CLI / SDK 默认。映射：Lilia composer model 直接写入 `options.model`。 | 已接入 |
| `PermissionMode` | Lilia 对读写、命令、网络和工具确认的统一权限模式。 | 原生语义：Codex thread / turn settings 可配置 approval、sandbox、permissions、permission profile；命令审批事件可携带可选 decision 和额外权限请求。映射：Lilia `full` / `ask` / `readonly` 映射到 Codex approval、sandbox、permissions 和 command exec profile；所有等待用户的审批仍回到 Lilia `tool_consent` / `permission_approval`。 | 原生语义：Claude `options.permissionMode` 控制 SDK 权限策略，`options.allowDangerouslySkipPermissions` 允许 bypass，`options.canUseTool` 是每次工具使用前的用户态决策回调。映射：Lilia mode 写入 permission options；工具级允许 / 拒绝通过 `canUseTool` 回到 Lilia interaction。 | 已接入 |
| `planMode` | 要求 agent 先给计划并等待 Lilia `plan_approval`，确认前不得执行修改。 | 原生语义：Codex `collaborationMode/list` 返回 collaboration presets；`turn/start.collaborationMode` 可把本轮设为 `plan`，计划事件以 plan / todo item 流上报。映射：Lilia 先读取 plan preset，首轮传 `mode: "plan"`，完成后展示 `plan_approval`，确认后下一轮显式传 default。 | 原生语义：Claude `options.permissionMode = "plan"` 进入计划模式，模型通过 `ExitPlanMode` 工具请求离开计划并进入执行。`Query.setPermissionMode()` 可在运行中切换权限模式。映射：`ExitPlanMode` 转为 Lilia plan timeline 和 `plan_approval`；同意后调用 `setPermissionMode()` 恢复执行权限。 | 已接入 |
| `conversationContext` / attachment context | 把 Lilia task、相关任务、附件、项目上下文和用户引用传给 provider。 | 原生语义：Codex `turn/start.additionalContext` / `turn/steer.additionalContext` 向当前 turn 附加非用户 prompt 的上下文；runtime workspace roots 和 permissions 限定可见项目边界。映射：Lilia runner payload 中的上下文进入 Codex additional context 和 profile roots。 | 原生语义：Claude SDK 可通过 MCP server / tool 暴露外部上下文；tool 调用返回内容由 Claude 当作工具结果读取。映射：Lilia 用 `createSdkMcpServer` / `tool()` 注册 `mcp__lilia__query_conversation_context`，Claude 调用时读取统一 conversation context。 | 已接入 |
| runtime session checkpoint | 记录 Lilia task 与 provider 原生 session 的绑定，保证后续 turn 接回同一会话。 | 原生语义：Codex thread id 是 app-server 会话标识；`thread/resume` 用该 id 恢复 thread。映射：Lilia 保存 `threadId`，后续 turns 先 resume 再 start turn。 | 原生语义：Claude SDK result / session state 中的 `session_id` 标识 transcript；`options.resume` 用该 id 恢复 session。映射：Lilia 保存 `session_id`，后续 query 写入 `options.resume`。 | 已接入 |
| runtime waiting state | 表示 provider 正等 Lilia UI 返回用户输入或审批。 | 原生语义：Codex `thread/increment_elicitation` / `thread/decrement_elicitation` 增减 out-of-band elicitation 计数，用于告知 thread 有外部用户交互挂起。映射：Lilia pending interaction 是权威状态；Codex adapter 在进入 / 退出 UI 等待时成对调用计数接口，失败只写 diagnostic。 | 原生语义：Claude SDK 没有等价等待计数 RPC；等待发生在 `canUseTool`、MCP tool 或 plan permission 回调内。映射：只维护 Lilia pending interaction，不写 provider 状态。 | 已接入 |
| `AgentTimelineEvent` | Lilia 的唯一事实流，用于消息、计划、工具、诊断、错误、goal、session 状态展示和持久化。 | 原生语义：Codex app-server 以 turn item、plan / todo、assistant message、command approval、file change、goal update、diagnostic、error 等事件描述执行过程。映射：adapter 按事件类型归一为 Lilia timeline，无法识别时保留摘要。 | 原生语义：Claude SDK 输出 `assistant`、`result`、`user`、`tool_result`、`stream_event` text / thinking、system / tool events 等消息。映射：adapter 将 SDK message stream 归一为 Lilia timeline，并保留 thinking、tool progress、permission denied 等系统事件。 | 已接入 |
| `AgentTimelineEvent(kind=tool)` 兜底 | 展示 provider 未登记工具或未知 item，保证原始执行信息不丢。 | 原生语义：Codex 可能新增未登记 item / tool 类型。映射：保留 provider tool / item name、input、status、summary 后走 Lilia 通用 `tool`。 | 原生语义：Claude 可能调用 Lilia 未专门归一化的内置工具或 MCP 工具。映射：保留 tool name / input / result 摘要后走 Lilia 通用 `tool`。 | 已接入 |
| interrupt / queue / rollback | 管理 Lilia turn 生命周期和 UI 一致性，provider 只执行当前被调度的 turn。 | 原生语义：Codex app-server 执行单个 active turn；中断、排队、回滚不是本接口的应用层状态来源。映射：Lilia Rust chat runtime 负责中断 / 队列 / rollback，Codex adapter 只消费当前 runner payload。 | 原生语义：Claude SDK query 是当前 runner 进程内的执行流；Lilia 当前通过终止 runner 子进程而不是 `options.abortController` 打断。映射：Lilia runtime 管理队列和 rollback，Claude adapter 只消费当前 query。 | 已接入 |

### Workflow

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `lilia_review` | 对指定代码范围、提交或未提交改动进行审查，并把结果写回 Lilia timeline。 | 原生语义：Codex `review/start` 在既有 thread 上启动 review，参数包含 `threadId`、`target`、`delivery`；target 表示审查范围，delivery 表示 inline / detached 交付方式。映射：Lilia `LiliaReviewTarget` 映射为 Codex target，delivery 映射为 Codex delivery；不再向 `review/start` 传自定义 prompt。 | 原生语义：Claude SDK 无 review 专用 API；`query()` 只接收普通 prompt 和 options。映射：Lilia 构造结构化 review prompt，带 target、instructions、delivery 语义进入 `query()`，结果仍按 Lilia review timeline 展示。 | 已接入 |
| `lilia_fix_suggestion` | 根据 review target 生成修复建议，或在用户允许时进入应用修复流程。 | 原生语义：Codex 无同名 fix suggestion API；普通代码修改只能通过 turn prompt 和工具执行。映射：Lilia 构造 fix suggestion prompt 经 `turn/start` 执行；`suggest` 模式只产出建议，`apply` 模式仍走权限和 tool consent。 | 原生语义：Claude 无同名 API；修复由 `query()` 中的 prompt 驱动，并由 Claude Code 工具执行。映射：Lilia 构造结构化 fix prompt，明确 target、mode、instructions 后进入 `query()`。 | 已接入 |
| `lilia_batch_apply` | 将 review / fix suggestion 的结果批量应用到工作区，必须保留 source 和用户确认边界。 | 原生语义：Codex 无 batch apply API；应用改动只能通过普通 turn、Plan 和工具执行。映射：Lilia 从 source turn / source kind / summary 构造 batch apply prompt，强制先进入 Plan 确认，再通过 Codex turn 应用。 | 原生语义：Claude 无 batch apply API；批量应用由 prompt 驱动的 `query()` 和工具执行。映射：Lilia 构造 batch apply prompt，要求 Claude 基于 source summary 生成并应用改动，权限仍由 Lilia 控制。 | 已接入 |
| `lilia_goal` | 设置、刷新或清除当前 Lilia task 的目标状态。 | 原生语义：Codex `thread/goal/set` 设置 objective / status / tokenBudget，`thread/goal/get` 读取当前 goal，`thread/goal/clear` 清除；server 会发 `thread/goal/updated` / `cleared`。映射：Lilia action `set` / `refresh` / `clear` 分发到对应方法，事件写回 goal timeline。 | 原生语义：Claude SDK 无 thread goal API。映射：Lilia goal timeline 和 task 状态作为权威；Claude adapter 写 diagnostic / timeline，不把 goal 注入为 provider 原生状态。 | 已接入 |
| `lilia_compact` | 手动压缩当前 provider 会话上下文，并记录压缩结果。 | 原生语义：Codex `thread/compact/start` 请求 app-server 压缩指定 thread；`thread/compacted` 表示完成。映射：Lilia 调用 `{ threadId }` 并监听完成 / 失败写 timeline，不做自动阈值触发。 | 原生语义：Claude Code 支持 `/compact` 斜杠命令压缩当前 transcript。映射：Lilia 向当前 Claude session 发送 `/compact`，结果写回 Lilia timeline。 | 已接入 |
| `lilia_background_terminals_clean` | 清理当前 session 相关后台终端，作为用户手动维护动作。 | 原生语义：Codex `thread/backgroundTerminals/clean` 清理指定 thread 的 background terminals。映射：Lilia workflow 调用该方法并把结果写 diagnostic，不接管终端生命周期。 | 原生语义：Claude SDK 无后台终端清理 API。映射：Lilia 写 unsupported diagnostic，说明未修改 Claude 运行时状态。 | 已接入 |
| `lilia_memory_mode` | 启用或关闭 provider 记忆模式。 | 原生语义：Codex `thread/memoryMode/set` 设置指定 thread 的 memory mode，参数为 `enabled` / `disabled`。映射：Lilia workflow 直接调用该方法并写 diagnostic。 | 原生语义：Claude SDK 无 memory mode 开关。映射：Lilia 写 unsupported diagnostic，不模拟 Claude 记忆状态。 | 已接入 |
| `lilia_memory_reset` | 重置 provider 记忆，但不删除 Lilia task history。 | 原生语义：Codex `memory/reset` 重置 Codex 自身全局 / 账户记忆。映射：Lilia 经二次确认触发，结果写 diagnostic，不在普通 turn 自动调用。 | 原生语义：Claude SDK 无 memory reset API。映射：Lilia 写 unsupported diagnostic，明确未重置 Claude transcript 或外部记忆。 | 已接入 |
| `lilia_session_fork` | 从当前 provider session 派生新 session，并把后续 Lilia turns 绑定到新 session。 | 原生语义：Codex `thread/fork` 复制 thread，上送 `excludeTurns`、runtime roots、permissions 等，返回新 thread id。映射：Lilia 默认 `excludeTurns: true`，成功后把新 id 作为 `done.sessionId` 和 checkpoint。 | 原生语义：Claude SDK `forkSession()` 从当前 session 分叉并返回新 `sessionId`。映射：Lilia 调用后把新 `sessionId` 写入 `done.sessionId` 和 checkpoint。 | 已接入 |
| `lilia_config_diagnostics` | 读取 provider 配置和运行要求，生成只读诊断摘要。 | 原生语义：Codex `config/read` 读取 effective config、apps、origins、layers；`configRequirements/read` 读取 reviewers、hooks、network 等要求。映射：Lilia 并行读取、压缩字段后写 diagnostic，不修改配置。 | 原生语义：Claude SDK 无 app-server 风格只读 config diagnostics；settings / managedSettings 是 query options 或外部文件来源，不是当前已接入读取面。映射：Lilia 写 unsupported diagnostic。 | 已接入 |
| `automation` | 自动化触发一次 Lilia agent turn，并把结果回写 automation run。 | 原生语义：Codex 没有 automation 原生概念。映射：automation 只产生 Lilia runner payload，Codex adapter 按普通 turn 执行。 | 原生语义：Claude SDK 没有 automation 原生概念。映射：automation 只产生 Lilia runner payload，Claude adapter 按普通 `query()` 执行。 | 已接入 |
| `slash_command` | 执行 Lilia native / project slash command，命令先解析为 Lilia workflow 或普通 turn。 | 原生语义：Codex app-server 可执行普通 turns；slash command 不是 provider 应用入口。映射：Lilia 解析 command 后再交给 Codex adapter。 | 原生语义：Claude Code 有自身 slash commands，但应用层不直接暴露 provider slash 作为协议。映射：Lilia slash command 先转为 Lilia workflow；需要 Claude 原生命令时作为 prompt / command 内容进入 `query()`。 | 已接入 |

### 用户交互

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `ask_user` | agent 主动向用户提问，UI 返回结构化答案、取消或拒绝。 | 原生语义：Codex `thread/start.dynamicTools` 可注册运行时工具；`AskUserQuestion` 被模型调用时 app-server 进入外部工具请求。映射：Lilia 注册 AskUser dynamic tool，调用转为 `interaction_request(kind=ask_user)`，用户结果回灌给 Codex。 | 原生语义：Claude SDK 可通过 `createSdkMcpServer` 和 `tool()` 注册 in-process MCP tool；`toolAliases` 可把友好工具名映射到 MCP tool 名。映射：Lilia 注册 `mcp__lilia__ask_user_question`，并设置 `options.toolAliases.AskUserQuestion`，调用转为 Lilia `ask_user`。 | 已接入 |
| `plan_approval` | 计划完成后等待用户同意、修订或取消，确认前不执行计划。 | 原生语义：Codex plan turn 自然完成后只产生 plan / todo 事件，执行需要新的 default turn。映射：Lilia 基于 plan timeline 展示 approval；同意后发起下一轮 default 执行，修订则继续 plan mode。 | 原生语义：Claude `ExitPlanMode` 是模型请求退出计划模式的工具，`Query.setPermissionMode()` 可恢复执行权限。映射：`ExitPlanMode` 触发 Lilia `plan_approval`；同意后切回执行权限，修订继续计划。 | 已接入 |
| `tool_consent` | 用户审批工具、命令、文件变更或可编辑命令输入。 | 原生语义：Codex command / file approval 事件请求用户批准；`item/commandExecution/requestApproval` 可带 `additionalPermissions`、`availableDecisions`、网络 / exec policy amend。映射：Lilia 归一成 `ToolConsentInteractionPayload`，返回 allow / deny / edit / provider decision。 | 原生语义：Claude `options.canUseTool` 在工具调用前向 host 请求允许 / 拒绝；只读限制需要 host 自行执行。映射：Lilia 在 `canUseTool` 内展示 tool consent，只读模式拒绝非白名单工具并写 timeline。 | 已接入 |
| edited command result | 用户编辑 provider 提议命令后，由 Lilia 执行修改版并把结果回灌给 provider。 | 原生语义：Codex `command/exec` 在 app-server host 上执行命令并返回结果；`process/spawn` 启动独立进程并用 output / exited 事件返回；`turn/steer` 给当前 turn 注入额外上下文。映射：Lilia 优先用 `command/exec` 执行编辑后命令，失败降级 `process/spawn`，然后通过 `turn/steer.additionalContext` 回灌退出码和输出摘要。 | 原生语义：Claude `options.hooks` 可注册 `PostToolUse` / `PostToolUseFailure` 等 hook，在工具执行后提供额外上下文。映射：Lilia 用 hooks 把用户修改命令的 additional context 提供给后续 Claude 执行；没有独立 command exec provider API。 | 已接入 |
| `permission_approval` | 审批 provider 提出的权限扩展请求，返回批准后的权限和 scope。 | 原生语义：Codex permission approval 可表达新增 permissions、scope 和 strict auto review 等请求。映射：Lilia 转为 `interaction_request(kind=permission_approval)`，用户结果回写 Codex。 | 原生语义：Claude SDK 无等价 permission approval RPC；权限只通过 `permissionMode`、`canUseTool`、allowed / disallowed tool settings 表达。映射：当前以 Lilia `PermissionMode` + `tool_consent` 承接。 | 已接入 |
| `mcp_elicitation` | MCP server 请求用户填写表单、打开 URL 或确认外部授权。 | 原生语义：Codex MCP elicitation 事件可表达 form / URL 请求，并等待 accept / decline / cancel。映射：Lilia 转为 `mcp_elicitation` interaction，用户结果回 Codex。 | 原生语义：Claude `options.onElicitation` 是 SDK 的 MCP elicitation host 回调，用于处理 server 发起的 elicitation。映射：当前未接入 Claude 侧，落到预设 `lilia_mcp_elicitation` 的 unsupported diagnostic；不能绕过 Lilia 直接弹 provider UI。 | 部分接入 |
| `architecture_change` | agent 请求修改 Lilia 项目架构图，用户确认后写入 Lilia architecture 数据。 | 原生语义：Codex 无 architecture 原生接口；只能通过工具调用来源触发。映射：Lilia 内置 architecture tool 触发统一 interaction，Codex 只作为调用来源。 | 原生语义：Claude 通过内置 MCP tool 调用外部 host 能力。映射：`mcp__lilia__update_project_architecture` 触发 Lilia `architecture_change` interaction，Claude 只作为调用来源。 | 已接入 |

### 历史与 session

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `HistoryImport*` search | 搜索可导入的 provider 历史，并返回 Lilia 可展示摘要。 | 原生语义：Codex `thread/search` / `thread/list` 搜索 threads。映射：内部 adapter 转为 `HistoryImportItem`，旧 `CodexThread*` 类型不向前端暴露。 | 原生语义：Claude Code 本地历史存储在 `~/.claude/projects/*.jsonl`。映射：内部 adapter 解析 JSONL 并转为 `HistoryImportItem`，旧 `ClaudeSession*` 类型不向前端暴露。 | 已接入 |
| `HistoryImport*` preview | 预览 provider 历史，映射为 Lilia messages / timeline event 计数。 | 原生语义：Codex `thread/turns/list` 和 `thread/turns/items/list` 读取 turns / items。映射：内部 adapter 转为 `HistoryImportPreview`。 | 原生语义：Claude 本地 JSONL 记录 session messages、tool events、results 等。映射：内部 adapter 转为 `HistoryImportPreview`。 | 已接入 |
| `HistoryImport*` attach | 将 provider 历史绑定到 Lilia task，创建 checkpoint 并后台同步。 | 原生语义：Codex thread id 可作为后续 resume checkpoint。映射：内部 adapter 绑定 thread 并返回 `HistoryImportAttachResult`。 | 原生语义：Claude session id 可作为 `options.resume` checkpoint。映射：内部 adapter 绑定 session 并返回 `HistoryImportAttachResult`。 | 已接入 |
| runtime state list | 查询 Lilia 运行时中 provider session 对应的 running / queued / pending 状态。 | 原生语义：Codex 没有 Lilia task runtime 状态列表；thread 状态不等同于 Lilia runner 队列。映射：`codex_thread_runtime_states` 从 Lilia store 和 chat runtime 推导。 | 原生语义：Claude 没有独立 runtime state list。映射：使用同一 Lilia chat runtime snapshot。 | 已接入 |
| task archive sync | Lilia task 归档后，best-effort 同步 provider 历史归档状态。 | 原生语义：Codex `thread/archive` 归档 provider thread。映射：Lilia task archive 后 best-effort 调用，失败只记录 diagnostic / log，不回滚 Lilia archive。 | 原生语义：Claude `deleteSession()` 是删除而不是 Lilia archive；当前未接入 rename / tag / archive 类 SDK session 管理。映射：Lilia task archive 是唯一应用层状态，不调用 Claude 删除。 | 已接入 |
| task title sync | Lilia task 标题更新后，best-effort 同步 provider session 标题。 | 原生语义：Codex `thread/name/set` 更新 thread 名称。映射：Lilia title 更新后 best-effort 调用，并写 diagnostic。 | 原生语义：Claude `renameSession()` 可改 session 名称但当前未接入。映射：Lilia task title 是唯一状态，不调用 Claude rename。 | 已接入 |
| provider session management | 预设完整 provider session 管理能力的当前子集。 | 原生语义：Codex threads 支持 search、list、turns、fork、archive、name set；remote / realtime 管理另属其他能力。映射：`lilia_session_management` 接入 list / info / messages / rename / archive，tag / delete 返回 unsupported diagnostic。 | 原生语义：Claude SDK 提供 `listSessions()`、`getSessionMessages()`、`getSessionInfo()`、`renameSession()`、`tagSession()`、`deleteSession()` 等 session API。映射：`lilia_session_management` 接入 list / info / messages / rename / tag / delete，archive 返回 unsupported diagnostic。 | 已接入 |

### 扩展与工具

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `AgentRuntimeExtensions` | 当前 turn 可用扩展集合，作为 Lilia runner payload 的一部分。 | 原生语义：Codex MCP server 配置来自 Codex config，adapter 可读取 server command、args、env、enabled 等。映射：Lilia 读取 `CodexRuntimeExtensions.mcpServers` 和 `codexConfigPath`，注入 Codex adapter。 | 原生语义：Claude SDK options 可接 `skills`、`plugins`、`mcpServers`。映射：Lilia 读取 `ClaudeRuntimeExtensions.skills`、`plugins`、`mcpServers` 并写入 SDK options。 | 已接入 |
| `PluginsOverview` | 插件页统一展示 Claude / Codex 扩展状态、配置路径和 warnings。 | 原生语义：Codex 当前可管理 MCP server 配置文件。映射：overview 返回统一 `mcpServers` 与 `configPaths.codex`。 | 原生语义：Claude 可发现 Skills、local Plugins 和 MCP config。映射：overview 返回统一 `skills`、`packages`、`mcpServers` 与 `configPaths.claude`。 | 已接入 |
| `PluginMcpServer` / `PluginMcpServerInput` | 管理 provider MCP server 配置，并作为 runtime extension 输入。 | 原生语义：Codex MCP server 配置定义 server name、command、args、env、enabled 等。映射：Lilia 通过 `plugins_*_mcp_server` 命令携带 backend 维护 Codex config。 | 原生语义：Claude `options.mcpServers` 注册外部 MCP server。映射：同一组 `plugins_*_mcp_server` 命令携带 backend 维护 Claude MCP config。 | 已接入 |
| `PluginSkill` | 管理 provider skill 能力，并在启用后传给 runtime。 | 原生语义：Codex 没有 Claude Skills。映射：Codex 当前返回空集合 / unsupported。 | 原生语义：Claude `options.skills` 指定启用的 user / project skill。映射：Lilia 通过统一 skill 命令管理，运行时写入 `options.skills`。 | 已接入 |
| `PluginPackage` | 管理 provider plugin/package 能力，并在启用后传给 runtime。 | 原生语义：Codex 当前没有 package 管理面。映射：Codex 返回空集合 / unsupported。 | 原生语义：Claude `options.plugins` 接收 plugin config，例如本地 path。映射：Lilia 通过统一 package 命令管理，运行时写入 `options.plugins`。 | 部分接入 |
| 内置 AskUser / context / architecture tools | Lilia 内置工具集合，向 provider 暴露 AskUser、conversation context 和 architecture change。 | 原生语义：Codex `thread/start.dynamicTools` 允许动态注册工具；additionalContext 可提供非工具上下文。映射：AskUser 通过 dynamic tool；context / architecture 由 runner payload 和 Lilia interaction 承接。 | 原生语义：Claude `createSdkMcpServer` + `tool()` 暴露 in-process MCP tools；`options.toolConfig.*.previewFormat` 控制工具预览格式。映射：注册 `ask_user_question`、`query_conversation_context`、`update_project_architecture`，preview format 固定为 `markdown`。 | 已接入 |
| external MCP | 用户配置的外部 MCP server 注入 provider runtime。 | 原生语义：Codex 从 Codex config 加载 MCP server。映射：Lilia Codex MCP 管理面写配置，运行时进入 `CodexRuntimeExtensions`。 | 原生语义：Claude `options.mcpServers` 注入外部 server；server 名 `lilia` 被内置 server 保留。映射：Lilia Claude MCP 管理面写配置，运行时注入非 `lilia` server。 | 部分接入 |
| hooks / config requirements | provider hooks / requirements 只作为 Lilia 诊断或内部实现，不形成独立应用入口。 | 原生语义：Codex `configRequirements/read` 返回 reviewers、hooks、network 等配置要求。映射：只进入 `lilia_config_diagnostics`，不提供绕过 Lilia 的 hooks 管理。 | 原生语义：Claude `options.hooks` 注册工具生命周期 hook；`includeHookEvents` 可影响 hook 事件输出。映射：当前仅用于 edited command additional context，hook lifecycle 事件作为 timeline 兜底，不提供 hooks 管理入口。 | 部分接入 |
| `lilia_provider_settings` | 高级 provider 设置统一入口，当前支持 diagnose / update。 | 原生语义：Codex `thread/settings/update` 可更新 model、approval、sandbox、permissions、reasoning effort、runtime roots、persistExtendedHistory 等。映射：Lilia `common.model` / `common.permission` 与 `codex.profile`、`reasoningEffort`、`permissionProfile`、`runtimeWorkspaceRoots`、`persistExtendedHistory` 进入 Codex thread settings。 | 原生语义：Claude SDK options 可接 allowedTools、disallowedTools、additionalDirectories、maxTurns、maxBudgetUsd 等字段。映射：Lilia 将这些 Claude 高级字段注入当前 SDK query options；`common.model` / `common.permission` 同步影响本轮 model / permission。 | 已接入 |
| thinking / reasoning | 模型推理强度和 reasoning / thinking 展示。 | 原生语义：Codex settings / collaboration mode 可包含 reasoning effort。映射：Lilia composer / profile 控制 Codex reasoning effort。 | 原生语义：Claude `stream_event` 可包含 thinking；SDK 还存在 `thinking`、`effort`、`maxThinkingTokens` 等配置面。映射：当前只把 thinking stream 写入 Lilia timeline，配置面的 Lilia 落点是 `lilia_provider_settings`。 | 部分接入 |
| subagents / task agent | provider 子 agent / task 事件的展示和未来管理落点。 | 原生语义：Codex plan / todo / item 事件描述内部计划与任务，不是可管理 subagent API。映射：只镜像到 Lilia timeline。 | 原生语义：Claude `Task` / `Agent` 工具可启动子任务，并有 progress、notification、subagent transcript 相关事件 / API。映射：当前只镜像调用、进度和通知；transcript 查询落到 `lilia_session_management`，主动调度和定义管理落到 `lilia_experimental_capability`。 | 部分接入 |

## 预设 Lilia 协议落点

以下能力当前未进入产品主流程，但已经预留 Lilia 落点名称。应用层若要接入，必须使用这些 Lilia 落点；Codex / Claude adapter 只能实现或降级这些落点，不能暴露 provider 专属入口。

| 预设 Lilia 落点 | Lilia 能力边界 | Codex 原生语义与映射 | Claude 原生语义与映射 |
|---|---|---|---|
| `lilia_realtime_session` | 语音 / realtime 文本会话：start、append audio、append text、stop、list voices，并以 Lilia session id 管理生命周期。 | 原生语义：Codex `thread/realtime/start` 启动 thread-scoped realtime session，参数包括输出模态、prompt、transport、voice；`appendAudio` / `appendText` 追加输入；`stop` 结束会话；`listVoices` 返回可用声音。映射：这些方法的 Lilia 落点是 `lilia_realtime_session`；当前返回 unsupported diagnostic。 | 原生语义：当前 Claude adapter 无稳定 realtime session API。映射：返回 unsupported diagnostic，不新增 Claude 专属入口。 |
| `lilia_remote_environment` | 远程控制、远程环境注册和远程状态诊断，所有状态回到 Lilia remote environment model。 | 原生语义：Codex `remoteControl/enable` / `disable` 开关 remote control，`remoteControl/status/read` 返回连接状态、serverName、installationId、environmentId；`environment/add` 注册外部 execution environment。映射：这些方法的 Lilia 落点是 `lilia_remote_environment`；当前返回 unsupported diagnostic。 | 原生语义：Claude `/bridge` 的 `attachBridgeSession`、`createCodeSession`、`fetchRemoteCredentials` 等 remote control 接口仍为 alpha。映射：当前返回 unsupported diagnostic；试验入口是 `lilia_experimental_capability`。 |
| `lilia_process_session` | 通用进程会话：spawn、stdin、kill、resize、output stream，进程归属和输出必须由 Lilia 管理。 | 原生语义：Codex `process/spawn` 在 app-server host 启动独立进程，输出通过 `process/outputDelta`、`process/exited` 返回；`writeStdin` 写入 / 关闭 stdin；`kill` 终止；`resizePty` 调整 PTY。映射：当前仅 edited command fallback 内部可用 `process/spawn`；应用层调用该落点返回 unsupported diagnostic。 | 原生语义：Claude SDK 无 provider 级 process API；命令执行由 Claude Code Bash tool 和 host 权限控制。映射：返回 unsupported diagnostic，命令仍走 agent tool + Lilia tool consent。 |
| `lilia_file_search_session` | provider 级 fuzzy file search：session start、query update、stop，结果需回到 Lilia 文件选择 / 附件模型。 | 原生语义：Codex `fuzzyFileSearch/sessionStart` 以 `sessionId`、`roots` 开始 legacy fuzzy search，`sessionUpdate` 更新 query，`sessionStop` 结束 session。映射：当前由 Lilia 自有上下文搜索 / 附件入口替代；若接入必须落到该预设。 | 原生语义：Claude SDK 无 provider 级 fuzzy file search session API。映射：使用同一 Lilia 上下文搜索 / 附件入口替代。 |
| `lilia_mcp_elicitation` | MCP elicitation 完整管理：form、URL、OAuth / auth、tool policy，并统一返回 accept / decline / cancel。 | 原生语义：Codex MCP elicitation 已可表达 form / URL 并等待用户响应。映射：现有 `mcp_elicitation` 是该落点已接入子集。 | 原生语义：Claude `options.onElicitation` 是 MCP elicitation host 回调。映射：该回调的 Lilia 落点是 `lilia_mcp_elicitation`；当前返回 unsupported diagnostic。 |
| `lilia_session_management` | provider session 全管理：list、messages、info、rename、tag、delete、archive，且不得取代 Lilia task 作为应用层权威状态。 | 原生语义：Codex threads 支持 `thread/search`、`thread/list`、`thread/turns/list`、`thread/turns/items/list`、`thread/archive`、`thread/name/set`。映射：当前接入 list / info / messages / rename / archive；tag / delete 返回 unsupported diagnostic。 | 原生语义：Claude SDK 提供 `listSessions()`、`getSessionMessages()`、`getSessionInfo()`、`renameSession()`、`tagSession()`、`deleteSession()`。映射：当前接入 list / info / messages / rename / tag / delete；archive 返回 unsupported diagnostic。 |
| `lilia_experimental_capability` | alpha / beta 能力的显式试验入口，必须记录能力名、provider、风险级别、默认关闭策略和回退语义。 | 原生语义：Codex experimental schema 可能包含 mock / raw events / remote / realtime / environment 等高变动方法和字段。映射：测试专用 `mock/experimentalMethod` 不接入；其他试验能力必须先挂到该落点或更具体的预设落点。 | 原生语义：Claude `options.betas`、`sessionStore`、`taskBudget`、assistant worker、experimental agent fields、bridge remote control 等为 beta / alpha / experimental。映射：如需试验必须挂到该落点并默认关闭，不能直接作为应用层入口。 |

### 字段级语义补充

本节只补充容易在实现时误读的 provider 字段。每一行仍然以 Lilia 落点为入口；字段没有产品入口时，落到预设 Lilia 落点并返回 unsupported diagnostic。

| Lilia 落点 | Lilia 接口含义 | Codex 原生语义与映射 | Claude 原生语义与映射 | 状态 |
|---|---|---|---|---|
| `chat_send_message` 普通 turn | provider 进程启动、工作目录、system prompt、流式输出和建议生成都属于一次 Lilia turn 的执行参数。 | 原生语义：Codex `thread/start` / `thread/resume` 的 `runtimeWorkspaceRoots` 限定运行时根目录，`permissions` 注入受控权限，`persistExtendedHistory` 控制扩展历史持久化；`thread/resume.initialTurnsPage` 可提供初始 turns 页。映射：已由 Lilia profile / checkpoint 注入；`history`、`path` 不作为应用层输入。 | 原生语义：Claude `options.cwd` 设置执行目录，`options.systemPrompt` 设置系统提示，`options.includePartialMessages` 输出 text / thinking partial，`options.promptSuggestions` 开启原生 prompt suggestion 事件，`pathToClaudeCodeExecutable` 可指定 Claude Code 可执行文件，`spawnClaudeCodeProcess` 可替换 SDK 启动子进程。映射：已接入 cwd、systemPrompt、partial、promptSuggestions；可执行文件和自定义 spawn 落到 `lilia_provider_settings`，当前 unsupported diagnostic。 | 部分接入 |
| `PermissionMode` | 工具白名单、黑名单、permission prompt 工具和运行时确认都必须统一回到 Lilia 权限模型。 | 原生语义：Codex `thread/settings/update` 可 sticky 更新 approval、sandbox、permissions、model、effort、summary、`collaborationMode`、personality 等后续 turn 设置；`item/commandExecution/requestApproval.availableDecisions` 表示 provider 允许的审批决策集合。映射：已接入 Lilia profile 和 tool consent；未产品化字段进入 `lilia_provider_settings`。 | 原生语义：Claude `allowedTools` 允许指定工具自动使用，`disallowedTools` 禁用工具，`tools` 可限制 / 替换内置工具基集，`permissionPromptToolName` 可改用 MCP permission prompt tool，`canUseTool` 是 host 决策回调。映射：当前只用 `canUseTool` + Lilia `tool_consent`；allow / deny / base tool / prompt tool 配置落到 `lilia_provider_settings`。 | 部分接入 |
| `lilia_provider_settings` | provider 设置来源、sandbox、结构化输出、预算和中断策略的统一设置入口。 | 原生语义：Codex `thread/start` / `turn/start` 的 `environments` 可绑定外部执行环境，`experimentalRawEvents` 可要求原始事件，`responsesapiClientMetadata` 传客户端元数据；`command/exec.permissionProfile` 控制 Lilia 执行编辑后命令的权限 profile。映射：metadata、command exec profile、environments 和 raw events 已接入；environments / raw events 只在 workflow 显式传入时受控透传。 | 原生语义：Claude `settings` 注入 flag settings，`managedSettings` 注入策略层设置，`settingSources` 控制 user / project / local 设置来源，`sandbox` 配置执行沙箱，`additionalDirectories` 扩展可访问目录，`outputFormat` 启用结构化输出，`maxTurns` / `maxBudgetUsd` 设置执行和费用上限，`abortController` 从 SDK 层中断 query。映射：这些稳定 SDK query options 已从 `lilia_provider_settings` 进入当前 SDK query；`sessionStore` 仅支持显式对象受控透传，不由 Lilia 自建 store。 | 已接入 |
| `lilia_session_management` | session 恢复、从某点恢复、继续最近 session 和 transcript 存储都必须由 Lilia task checkpoint 约束。 | 原生语义：Codex `thread/resume.excludeTurns` 可恢复时排除 turns，`thread/fork.excludeTurns` 控制 fork 是否排除 turns，`thread/resume.path` / `thread/fork.path` 可按路径恢复 / fork。映射：Lilia 只使用 checkpoint thread id 和默认 `excludeTurns` 策略；path 不作为应用层输入。 | 原生语义：Claude `options.resume` 恢复指定 session，`options.continue` 继续最近 session，`options.resumeSessionAt` 从指定 message UUID 恢复，`sessionId` 可指定新 session id；`sessionStore` / `sessionStoreFlush` / `loadTimeoutMs` 控制 transcript mirror 到外部 store 的加载和刷盘。映射：`resume` 仍由 Lilia checkpoint 驱动；continue / resumeSessionAt / sessionId 作为 `lilia_provider_settings` 受控 query options 接入；sessionStore 只支持显式对象受控透传，不取代 Lilia task。 | 已接入 |
| `lilia_mcp_elicitation` | MCP transport、elicitation、auth 和 tool policy 都必须以 Lilia interaction / extension 形式落地。 | 原生语义：Codex MCP 当前通过 Codex config 进入 runtime，elicitation 事件可回到 Lilia；HTTP / SSE / OAuth 等完整管理尚未产品化。映射：已接入 stdio 配置和 elicitation 子集。 | 原生语义：Claude `options.mcpServers` 支持外部 MCP server，SDK instance MCP 可由 `createSdkMcpServer` 提供，`options.onElicitation` 处理 server elicitation；MCP tool policy / OAuth / HTTP / SSE 需要独立配置。映射：已接入 stdio 和内置 SDK server；elicitation / auth / policy 落到 `lilia_mcp_elicitation`。 | 部分接入 |
| `AgentTimelineEvent` | hook、subagent、thinking 和系统状态事件只能作为 Lilia timeline / diagnostics 展示，不能绕过 Lilia 状态机。 | 原生语义：Codex plan / todo / command / file item 都是 app-server 事件；raw events 可受控请求但不成为应用层事实源。映射：已归一为 timeline，未知事件走 tool / diagnostic 兜底，raw events 仅用于诊断/调试输入。 | 原生语义：Claude `includeHookEvents` 控制是否输出全量 hook lifecycle，`forwardSubagentText` 转发 nested subagent 文本，`agentProgressSummaries` 生成子 agent 周期性摘要；`listSubagents()` / `getSubagentMessages()` 读取 subagent 列表和 transcript。映射：hook / subagent / thinking 事件镜像到 timeline；进度摘要和事件开关落到 `lilia_provider_settings`。 | 已接入 |
| `lilia_experimental_capability` | beta / alpha 能力必须显式标注试验范围，默认关闭，且有 Lilia 回退语义。 | 原生语义：Codex `mock/experimentalMethod` 只验证 experimental gating；`mockExperimentalField` 是测试字段。映射：测试字段不接入产品；其他 experimental 字段先落到更具体预设或本落点。 | 原生语义：Claude `options.betas` 传 SDK beta 开关，`AgentDefinition.criticalSystemReminder_EXPERIMENTAL` 是自定义 agent 的实验提醒字段，`taskBudget` 是 alpha 预算字段，`startup()` 返回预热 query handle，`/bridge` remote control 和 `/assistant` worker 都是 alpha / experimental surface。映射：当前返回 unsupported diagnostic；如需试验必须先走 `lilia_experimental_capability`，并写清关闭和 fallback 行为。 | 诊断替代 |

## 升级复核清单

升级 Claude SDK 或 Codex CLI 后，按以下顺序复核：

1. 对照 `ChatWorkflow`、`AgentInteractionKind`、history contracts、runtime extension contracts，确认本文每个 Lilia 接口仍有 provider 映射。
2. Codex：重新生成普通 schema 与 `--experimental` schema，比较新增 JSON-RPC 方法和同名类型新增字段；重点核对 `thread/start`、`thread/resume`、`turn/start`、`item/commandExecution/requestApproval`、`ConfigRequirements`。
3. Claude：核对 SDK `Options`、`PermissionMode`、`HookEvent`、`SDKMessage`、`SdkPluginConfig`、`bridge.d.ts`、`assistant.d.ts`。
4. 如果 provider 新增原生能力，先归入本文的预设 Lilia 落点；确实没有合适落点时，先新增 Lilia 落点名称和双 provider adapter 语义，再接入实现。
5. 旧 provider 文档已删除，不再维护独立权威状态表；需要追溯来源时查看 Git 历史。
