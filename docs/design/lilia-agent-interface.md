# Lilia Agent 三层协议

> 状态：本文是 Lilia 应用层到 Claude / Codex provider adapter 的协议边界文档。
> 核对时间：2026-06-19。

## 协议分层

Lilia 不再把所有 agent 行为都塞进 `ChatWorkflow`。当前协议分为三层：

```mermaid
flowchart LR
  UI["UI / task surface"] --> Intent["ChatWorkflow\n用户可见意图"]
  UI --> Runtime["ChatRuntimeCommand\n运行时控制"]
  Runtime --> Options["ProviderRuntimeOptions\nadapter 输入"]
  Intent --> Runner["runner stdin payload"]
  Runtime --> Runner
  Options --> Runner
  Runner --> Registry["provider adapter registry"]
  Registry --> Codex["Codex adapter"]
  Registry --> Claude["Claude adapter"]
```

`ChatWorkflow` 表达用户在 Lilia 里可见、可解释、可持久化的 agent 意图。`ChatRuntimeCommand` 表达会话、设置和未来 realtime / remote / process / file-search 这类运行时控制面。`ProviderRuntimeOptions` 只允许 runner 到 provider adapter 内部消费，不能作为 UI workflow。

## 输入形状

runner stdin 的稳定形状是：

```ts
{
  turn: {
    cwd: string;
    prompt: string;
    attachments: ChatAttachment[];
    model: string;
    resumeSessionId?: string | null;
    planMode: boolean;
    permission: PermissionMode;
  };
  workflow?: ChatWorkflow | null;
  runtimeCommand?: ChatRuntimeCommand | null;
  runtimeOptions?: ProviderRuntimeOptions | null;
}
```

旧输入 `lilia_session_fork`、`lilia_session_management`、`lilia_provider_settings` 不再作为 `workflow` 或 `runtimeCommand` 接收。调用方必须发送当前 `runtimeCommand`，provider 字段必须进入 `runtimeOptions.provider`。

## ChatWorkflow

`ChatWorkflow` 只保留用户可见工作流：

| workflow | 含义 | 空 prompt |
|---|---|---|
| `lilia_review` | 对指定代码范围做审查。 | 支持 |
| `lilia_fix_suggestion` | 生成修复建议或按模式应用修复。 | 支持 |
| `lilia_batch_apply` | 批量应用 review / fix suggestion 的结果。 | 支持 |
| `lilia_goal` | 设置、刷新或清除当前线程目标。 | 支持 |
| `lilia_compact` | 压缩当前 provider 会话上下文。 | 支持 |
| `lilia_background_terminals_clean` | 清理当前会话相关后台终端。 | 支持 |
| `lilia_memory_mode` | 启用或关闭 provider 记忆模式。 | 支持 |
| `lilia_memory_reset` | 重置 provider 记忆。 | 支持 |
| `lilia_config_diagnostics` | 读取 provider 配置和要求的诊断摘要。 | 支持 |
| `automation` | 自动化触发 agent turn。 | 支持 |
| `slash_command` | 执行 Lilia native / project slash command。 | 支持 |

空 prompt 规则来自 `packages/contracts/src/lilia-agent-protocol.json` 中按 kind 声明的 `requiresPrompt`。不要再新增散落字符串集合维护空 prompt workflow。

## ChatRuntimeCommand

`ChatRuntimeCommand` 是运行时控制入口：

| runtime command | 含义 | provider 映射 |
|---|---|---|
| `session_fork` | 从当前 provider session 分叉新 session。 | Codex 使用 thread fork；Claude 使用 session resume / transcript 能力时由 adapter 映射或 diagnostic。 |
| `session_management` | list / info / messages / rename / tag / delete / archive / history search / turn item list 等 provider session 管理。 | Codex 接 `thread/list`、`thread/search`、`thread/read`、`thread/turns/list`、`thread/turns/items/list`、`thread/archive`、`thread/name/set`；Claude 接 SDK session APIs。 |
| `runtime_settings` | diagnose / update provider runtime 设置。 | 设置值必须进入顶层 `runtimeOptions.common` / `runtimeOptions.provider`；Claude 写本地诊断并把 update 映射到 SDK query options；Codex 接 `thread/settings/update`，并消费 `thread/settings/updated` 作为状态同步。 |
| `remote_environment` | 注册或选择 provider 远程执行环境。 | Codex 接 `environment/add`；Claude 无等价能力时写 diagnostic。 |
| `process_session` | 管理 provider 独立进程 session，包括启动、stdin、终止、PTY resize、输出和退出事件。 | Codex 接 `process/spawn`、`process/writeStdin`、`process/kill`、`process/resizePty`，并消费 `process/outputDelta`、`process/exited`；Claude 无等价能力时写 diagnostic。 |
| `remote_control` | 管理 provider 远程控制启停和状态读取。 | Codex 接 `remoteControl/enable`、`remoteControl/disable`、`remoteControl/status/read`；Claude 无等价能力时写 diagnostic。 |
| `sandbox_diagnostics` | 读取 provider sandbox readiness / diagnostic。 | Codex 接 `windowsSandbox/readiness`；Claude 无等价能力时写 diagnostic。 |

预留 runtime command 边界包括 realtime 和 file search session。接入这些能力时必须先在本文定义 Lilia 协议名、层级和 fallback，再实现 provider 映射，不得扩大 `ChatWorkflow` union。

## ProviderRuntimeOptions

`ProviderRuntimeOptions.common` 只保存稳定 Lilia 字段：

| 字段 | 含义 |
|---|---|
| `model` | 模型选择。 |
| `permission` | Lilia 权限模式。 |
| `reasoningEffort` | 通用 reasoning / effort 意图。 |
| `runtimeWorkspaceRoots` | 运行时工作区根目录。 |
| `modelSelection` | Lilia 智能模型选择解释，只用于诊断和持久化，不作为 provider 原生命令参数。 |

provider 专属字段只能在 adapter 边界出现：

| provider | 字段示例 | 消费方 |
|---|---|---|
| `provider.codex` | `profile`、`reasoningEffort`、`runtimeWorkspaceRoots`、`persistExtendedHistory`、`initialTurnsPage`、`excludeTurns`、`environments`、`experimentalRawEvents`、`responsesApiClientMetadata` | `apps/desktop/agent-runner/codex/runCodex.mjs` |
| `provider.claude` | `reasoningEffort`、`thinking`、`allowedTools`、`disallowedTools`、`additionalDirectories`、`maxTurns`、`maxBudgetUsd`、`tools`、`settings`、`managedSettings`、`sandbox`、`outputFormat`、`sessionStore` | `apps/desktop/agent-runner/claude/runClaude.mjs` |

智能模型选择在发送前合并这些字段：显式 `runtimeOptions.common` / `runtimeOptions.provider.<provider>` 优先，其次是 composer 手动覆盖，最后才由自动策略填补缺省值。Claude `reasoningEffort` 会映射到 SDK `options.effort`，并在未显式传入 `provider.claude.thinking` 时补 `thinking: { type: "adaptive" }`；Codex `max` 会降级为 `xhigh` 并写入 `common.modelSelection.signals`。

高变动能力放入 `experimentalProviderOptions[]`。每项必须包含：

| 字段 | 规则 |
|---|---|
| `provider` | 目标 provider。 |
| `capability` | 稳定能力名，不使用 provider 方法名。 |
| `payload` | provider adapter 内部解释的输入。 |
| `fallback` | adapter 不认识时写 diagnostic / unsupported / ignore 中一种明确行为。 |

UI 禁止直接构造 provider 专属 payload。高级能力必须通过 `ChatRuntimeCommand` 或 `experimentalProviderOptions` 进入 adapter。

### Runtime Extensions

runtime extensions 是 provider 扩展能力的 Lilia 层入口，能力名必须稳定，不使用 provider 方法名：

| capability | Lilia 语义 | Codex 映射 |
|---|---|---|
| `permission_profiles` | 列出当前 cwd 可用的权限配置档案。 | `permissionProfile/list`。 |
| `plugin_inventory` | 读取 provider 已安装 plugin 清单。 | `plugin/installed`。 |
| `plugin_skill_read` | 读取远端 plugin 中某个 skill 的内容。 | `plugin/skill/read`。 |
| `plugin_share` | 保存、更新目标、列出、checkout 或删除 plugin share。 | `plugin/share/save`、`plugin/share/updateTargets`、`plugin/share/list`、`plugin/share/checkout`、`plugin/share/delete`。 |
| `skills_extra_roots` | 设置 provider skill 额外搜索根目录。 | `skills/extraRoots/set`。 |
| `account_quota_status` | 读取 provider 官方账号额度 / rate limit 状态，供连接状态、设置页和内部 quota 工具展示。 | `account/rateLimits/read`。 |

这些能力只允许从 runtime extensions 管理面或 `experimentalProviderOptions[]` 进入 adapter。UI 不直接拼 provider payload；adapter 不支持时按 `fallback` 写 diagnostic / unsupported / ignore。

## Adapter Registry

agent-runner 以 provider adapter registry 分发：

| registry 字段 | 要求 |
|---|---|
| `kind` | 声明 workflow 或 runtime command 类型。 |
| `supportsEmptyPrompt` | 来自协议 metadata。 |
| `handler` | 处理 Lilia 落点，不暴露 provider 方法名给 UI。 |
| `fallback` | provider 不支持时写 diagnostic / unsupported result。 |

公共 review / fix / batch / goal validation 放在共享模块。Codex / Claude adapter 只做 provider 映射和降级，不重复解析相同 Lilia workflow。

## Interaction 契约

`permission_approval` 使用 provider-neutral payload：

| 字段 | 含义 |
|---|---|
| `reason` | 向用户展示的权限扩展原因。 |
| `requestedAccess` | UI 可渲染的公共访问请求。 |
| `scopeSuggestion` | 可选的公共 scope 建议。 |
| `providerContext` | adapter round-trip 上下文，UI 不依赖其内部字段。 |

Codex 的 `threadId`、`turnId`、`itemId`、`strictAutoReview`、原始 permissions 等只放在 `providerContext.codex`，UI 只依赖公共字段渲染，提交时把 `providerContext` 原样传回 adapter。

用户编辑 Codex 命令后的 Lilia-owned 执行仍属于 `permission_approval` 的后续处理，timeline 使用 `command` / `subkind: "lilia_edit_exec"`。即使 Codex adapter 内部使用 `process/spawn` 兜底执行编辑后的命令，也不能把这个已实现功能改名或迁移成通用 `process_session`；只有独立进程控制 UI / API 才使用 `process_session`。

`attestation_request` 是 provider-initiated interaction，不属于用户可见 workflow。Codex 可把 `attestation/generate` 映射到该 interaction；当前 Lilia 若未实现，adapter 必须写明确 unsupported diagnostic，不能静默吞掉 request。

## 落点表

| 落点 | 层级 | Lilia 语义 | 不支持时 |
|---|---|---|---|
| 普通 turn | turn | 启动一轮 agent 输入，写 timeline 和 session checkpoint。 | 写 error timeline。 |
| review / fix / batch / goal | workflow | 用户可见 agent 工作流。 | 构造 Lilia prompt 或写错误。 |
| compact / memory / diagnostics | workflow | 用户触发的会话维护和诊断。 | 写 diagnostic。 |
| automation / slash command | workflow | 自动化或命令触发的 Lilia 行为。 | 拒绝启动并保留状态。 |
| session fork / management | runtime command | provider session 控制，不是 UI workflow。 | 写 unsupported diagnostic。 |
| provider settings | runtime command + runtime options | 诊断或更新 adapter runtime 设置，不污染 workflow；timeline payload 统一使用 `backend`、`subkind: "provider_settings"`、`action`、`settingsKeys`。 | 无有效字段时拒绝；未知 experimental capability 按 fallback。 |
| remote environment / process / remote control / sandbox diagnostics | runtime command | provider 运行时控制，不是 UI workflow。 | 写 unsupported diagnostic。 |
| permission approval | interaction | provider-neutral 权限审批。 | provider 无等价能力时走 `PermissionMode` / `tool_consent`。 |
| attestation request | interaction | provider 发起的 attestation 请求。 | 写 unsupported diagnostic。 |
| plugins / extensions / account quota | runtime extensions | 当前 turn 可用扩展集合、扩展管理和 provider 账号额度状态。 | 单项 warning，不阻塞其他扩展。 |

## 升级复核清单

1. 用户可见 agent 意图先判断是否属于 `ChatWorkflow`；session / settings / remote / realtime / process / file-search 默认属于 `ChatRuntimeCommand`。
2. 升级 Claude SDK 或 Codex CLI 后，先判断 provider 能力是否能落到已有 Lilia 协议；不能落地时，先在本文定义 Lilia 协议名、层级和 fallback，再实现 provider 映射。
3. provider 字段先放入 `ProviderRuntimeOptions.provider.<provider>` 或 `experimentalProviderOptions`，不得加到 public workflow。
4. runner payload 必须保持 `{ turn, workflow?, runtimeCommand?, runtimeOptions? }` 分层。
5. provider adapter 不认识 runtime command 或 experimental capability 时必须写 diagnostic / unsupported result。
6. UI 不直接读取或构造 providerContext 内部字段；只 round-trip 给 adapter。
7. 旧 provider 专属 flow 不是 Lilia 协议来源；升级后以本文的 Lilia 协议名为准，不按上游历史方法名倒推 UI contract。
