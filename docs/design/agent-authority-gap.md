# Agent 回合权威对照：LiliaCode 与 Mutsuki AgentKit

本文记录 Issue #69 的迁移边界和当前接线状态。它不是验收结论，也不表示 Issue 已关闭。当前实现中的应用入口是
`apps/desktop/src/application/agent.rs`；回合队列和内存协调器在
`crates/features/lilia-feature-agent-session/src/queue.rs` 与 `runtime.rs`，不再是旧的
`crates/lilia-desktop-application/src/agent.rs` 路径。

对照基准：Mutsuki pin rev `bb728d20`（见 [mutsuki-dependency-pin.md](mutsuki-dependency-pin.md)）。

## 当前结论

AgentKit 已成为单个 session 的回合事实来源：AgentKit session 事件、审批/交互等待 checkpoint、带版本的审批/交互恢复、取消入口，以及事件序列和订阅，都由
`crates/lilia-agent` 提供。LiliaCode 应用层不再有 `ActiveTurnPhase`，也不再为每个回合、审批或交互创建裸线程。

LiliaCode 仍然有产品层的跨回合职责：SQLite FIFO 队列、claim 所有权和恢复、失败重试、标题生成、auto-turn 档位、automation 关联、产品闸门和 payload 适配。`DesktopAgentRuntime` 是队列协调器，不是 AgentKit 回合状态机；它仍持有本地 claim 生命周期和终态后出队逻辑。

取消路径已经按事件序号刷新时间线：`restore_persisted_turn_queue`、`cancel_automation_agent_turn`、`apply_user_turn_cancellation` 在取消完成后，用该 session 最后一条事件序号发布 `TimelineChanged`。快照失败、会话没有事件或没有 session id 时拿不到序号，cursor 仍为 `None`，界面整段重读。

下列项目仍是未闭合缺口，不能据此宣称 Issue #69 已验收：

- `claim_token`/`claim_epoch` 仍是队列 ack 的所有权保护。当前代码把 worker 观测到的 `SessionVersion` 绑定到 claim，并在同一进程的 finish ack 校验 token+version；但恢复后的 version 仍来自 projection，AgentKit 尚未为 approval/interaction/restart 提供统一的 SessionVersion，因此上游权威没有完全闭合。
- Kernel Job payload 携带提交时捕获的不可变 `claim_token`。turn、审批、交互和 compaction 在触碰 AgentKit 前及页面/终态路径都会用该 token 复核 durable claim；发现旧 token 时丢弃旧 worker 结果，不发终态或恢复错误，也不清理新 owner，让恢复后的 owner 继续处理。缺少 token 的旧 Job 会被拒绝。
- AgentKit 的 `AgentSessionCoordinator` 尚未接入 loop plugin；`PermissionRequest.version` 仍可能来自 transcript message 长度，而不是 session version。恢复审批时必须透传请求携带的 version，不能由 LiliaCode 推导。
- 会话分叉已拆成两步：先由 AgentKit fork 创建目标 session，再 `bind_forked_task_session`。绑定失败时任务仍指向原 session。绑定失败留下的目标 session 尚未清理。
- `update_project_architecture` 已有 `PluginBuilder` 执行边界（`lilia.agent.project-architecture@1`）。插件只授权，不落图；确认落图仍在 `apps/desktop/src/application/agent_architecture.rs`。产品工具并没有全部改成 `AgentPluginRegistrar`。
- `LiliaCompact` 换会话并更换 task 绑定：摘要写入新的 durable session，提交成功后才把任务从压缩前的 session 改绑过去。取消或重启打断提交时，任务仍绑定压缩前的 session。AgentKit `compaction_service` 只压缩当次模型输入，不替换 durable transcript，因此 `build_product_coding_profile` 不打开它。
- 本次 Windows 验收的 `cargo xtask agent-debug` 仍未通过。抓帧已对准真实客户区，设置页截图能看出界面；门禁在用量趋势图提示未消失时退出，code 为 `quota_tooltip_not_cleared`。产物在 `agent-debug-runs/lilia-agent-debug-1791377960081`。陈旧 claim、重启恢复、审批版本和增量游标没有被这条未完成的门禁覆盖。

## 逐项对照

| # | 能力 | 当前代码与边界 | 处置/状态 |
|---|------|----------------|------|
| 1 | 回合状态机 | `ActiveTurnPhase` 已从仓库移除。`feature-agent-session::runtime::DesktopAgentRuntime` 只记录 active/queue、提交和 claim；应用快照只保留 `idle`/`starting`/`running`，再用 `lilia-agent::projection::AgentTurnCheckpoint` 投影 `waiting_approval`/`waiting_interaction`。 | **已拆除本地 FSM；持续检查投影一致性**。 |
| 2 | 单回合互斥 | `desktop_pending_turns` 的 `claim_token`、`claim_epoch` 以及内存 active 记录仍由 `queue.rs`/`runtime.rs` 使用。worker 观测到的 AgentKit `SessionVersion` 会在 claim 后写入 `sv:{n}`，同一进程 finish ack 校验 token+version；恢复后的 version 仍来自 projection，跨 approval/interaction/restart 的统一 SessionVersion 尚未由 AgentKit 提供。 | **保留**，直到上游提供统一的 ack fence；不得提前删除。 |
| 3 | 回合排队 | `crates/features/lilia-feature-agent-session/src/queue.rs` 提供 SQLite FIFO；`runtime.rs` 负责内存中的 active 与下一个 turn 晋级。 | **保留**，只负责跨回合提交顺序，不拥有 AgentKit 等待状态。 |
| 4 | 幂等提交 | 队列的 `enqueue_idempotent` 负责本地去重；automation key 也仍在队列行中。AgentKit wire 层负责 session 内提交的幂等语义。 | **部分改接**；需继续确保队列重放与 wire 幂等键一致。 |
| 5 | 显式取消 | `apps/desktop/src/application/agent.rs` 通过 `cancel_session_turn` 请求 AgentKit 取消，同时由本地 runtime/queue 清理或完成当前 claim。 | **AgentKit 负责回合取消；LiliaCode 保留队列清理语义**。 |
| 6 | 中断（回合中新输入） | 默认新输入进入 FIFO，不抢占 active turn；等待中的恢复仍走 AgentKit wire 的版本检查。 | **对齐**，但应继续验证 waiting turn 的陈旧恢复错误不会被本地队列吞掉。 |
| 7 | 失败重试 | `timeline_retry.rs` 读取 `retryContext`，用新的 turn id 重建请求；AgentKit 没有跨回合 `RetryTurn` API。 | **保留**，不引入第二套状态机。 |
| 8 | 审批 | `AgentTurnCheckpoint` 从 AgentKit 事件重建 AgentKit-owned pending；`agent.rs` 的审批入口把带 `action_revision` 的 decision 交给 AgentKit resume。产品 pending 表仍可作为可重建缓存。远程审批、交互和中断入口没有 durable claim 时必须先完成队列恢复，不再回退到 projection-only AgentKit 调用；保留的 projected 兼容方法也只转发到带 claim fence 的入口。 | **本地等待相位已删除；payload/版本适配仍在 LiliaCode**。 |
| 9 | 交互 | `waiting_interaction` 与 pending 同样来自 checkpoint；`tool_consent`、`mcp_elicitation`、`architecture_change` 由 LiliaCode 适配并在必要时先应用产品图。架构图 apply 与恢复提交都在 claim fence 下执行，旧 owner 不再发布等待或交互变化。 | **本地相位已删除；适配器和图应用保留**。 |
| 10 | 回合内上下文压缩 | AgentKit `compaction_service` 只压缩当次模型输入，不替换 durable transcript。`build_product_coding_profile` 不设置 `context.compaction_service`。`LiliaCompact` 仍由 `turn_run.rs` 交给 `AgentTurnHost::run_compaction`，再由 `agent.rs::run_context_compaction_turn` 执行。 | **不启用 profile compaction**；换会话压缩留在产品工作流。 |
| 10b | 显式换会话压缩 | `context_compaction.rs` 生成摘要并创建新 session，提交未被取消时才替换 task/session binding。取消或重启发生在绑定替换前时，任务仍指向压缩前的 session。 | **保留**。这是换会话并更换绑定，不是 AgentKit 单回合输入压缩。 |
| 11 | 自动续回合 | `finish_turn` 调用 feature queue 的 `ack_and_claim_next`；`auto_turn.rs` 仅作模型/档位选择。 | **保留**，属于跨回合产品编排。 |
| 12 | 标题更新 | `title_update.rs` 通过 `lilia.agent/title@1` Kernel Job 调度，启动时由 `desktop.rs` 安装 `QueuedTitleScheduler`。 | **已接 Kernel Job；不应恢复私有标题线程池**。 |
| 13 | 事件投影 | `agent_turn_host.rs` 的 observed 路径使用 `AgentEventEnvelope.sequence`。`restore_persisted_turn_queue`、`cancel_automation_agent_turn`、`apply_user_turn_cancellation` 在取消完成后用该 session 最后一条事件序号发布 `TimelineChanged`；拿不到序号时 cursor 仍为 `None`。 | **取消路径已带序号**；拿不到序号时仍整段重读。 |
| 14 | 工具注册 | `update_project_architecture` 已有 `PluginBuilder` 执行边界（`lilia.agent.project-architecture@1`），插件只授权、不落图；确认落图仍在 `apps/desktop/src/application/agent_architecture.rs`。worktree、memory、todo、automation 等产品工具并没有全部改成 `AgentPluginRegistrar`。 | **未闭合**；不得宣称产品工具已全部插件化。 |
| 15 | 持久化 | AgentKit transcript/checkpoint 是审批与交互事实；`desktop_pending_turns` 是跨回合队列，`pending_projections` 仍是可重建的产品/UI 缓存，`agent_session_bindings` 保留 task↔session 绑定。 | **收敛中**；不得把缓存表当恢复权威。 |
| 16 | 线程与执行 | 生产启动在 `desktop.rs` 安装 `QueuedTurnExecutor`，发 `Message::RequestTurnJob`/`RequestApprovalJob`/`RequestInteractionJob`，再由 Kernel Job 调用 `DesktopTurnPort::execute_*_job`。`agent.rs` 不再 spawn 回合/审批/交互线程；未安装 executor 的测试/无 host 路径是 caller-thread fallback。 | **生产接线已完成；Job 缺少 claim token 时拒绝执行**。 |
| 17 | 会话分叉 | 分叉已拆成两步：`turn_run.rs` 与 `fork_task_agent_session_through_turn` 先走 AgentKit fork 创建目标 session，再 `bind_forked_task_session`。绑定失败时恢复原 binding，任务仍指向原 session。绑定失败留下的目标 session 尚未清理。 | **两步已拆开**；失败留下的目标 session 仍待清理。 |

## 留在 LiliaCode 的产品语义

以下不属于 AgentKit 单回合事实，迁移后仍由产品 feature 负责：

- worktree 闸门和上下文注入（`worktree.rs`、`agent.rs`）。
- 提交/停止 hooks、任务运行闸门、slash command 本地执行。
- Guide/todo 派发、auto-turn 档位选择和 automation 节点关联。
- `timeline_retry.rs` 的失败重试（新 turn id）。
- `title_update.rs` 的标题生成 Kernel Job（`lilia.agent/title@1`）。
- `tool_consent.rs`、`mcp_elicitation.rs` 的载荷校验，以及 architecture interaction 的图应用后再恢复。
- `projection.rs` 的 AgentKit event → timeline/todo/artifact/pending 投影。它不写事实库，产品存储只能保存可重建缓存。

## 当前代码路径与接线

生产路径的协议定义在
`crates/features/lilia-feature-agent-session/src/execution.rs`：

- `lilia.agent/turn@1`
- `lilia.agent/approval@1`
- `lilia.agent/interaction@1`
- task slot `lilia.agent.turn.{task_id}`

`AgentSessionFeature`（`crates/features/lilia-feature-agent-session/src/lib.rs`）注册上述协议；`apps/desktop/src/kernel_host.rs` 挂载 feature 和 `TurnPort`。桌面启动在 `apps/desktop/src/desktop.rs` 安装 `QueuedTurnExecutor`，它只发 `Message::Request*Job`；`DesktopTurnPort` 在 Job worker 回调 `apps/desktop/src/application/agent.rs::execute_*_job`。这些执行入口再调用 `run_turn_worker`、`run_approval_worker` 或 `run_interaction_worker`，这里的 “worker” 是回调名称，不是裸线程。

恢复/投影路径为：`agent.rs::task_turn_checkpoint` →
`lilia-agent::checkpoint_from_session` → `task_runtime_snapshot` 与
`merge_task_pending`。AgentKit-owned pending kind（permission、ask-user、plan、tool consent、MCP elicitation、architecture change）由 checkpoint 重建，产品 pending 行不能覆盖仍开放的 AgentKit 请求。

## 缺口与补法

| 缺口 | 现状与补法 | 归属 |
|------|------------|------|
| 陈旧 worker ack | 当前代码把 worker 观测到的 version 绑定到 claim，同一进程 finish ack 校验 `claim_token`+version；`prepare_recovery` 必须换 token。恢复后的 version 仍来自 projection，跨 approval/interaction/restart 的统一 SessionVersion 尚未由 AgentKit 提供，因此本地 epoch/token 仍不能删除。 | LiliaCode + Mutsuki 协作 |
| 取消后的增量时间线 | `restore_persisted_turn_queue`、`cancel_automation_agent_turn`、`apply_user_turn_cancellation` 在取消完成后用 session 最后一条事件序号发布 `TimelineChanged`。拿不到序号时 cursor 仍为 `None`，界面整段重读。 | LiliaCode |
| AgentKit 无跨回合队列 | `feature-agent-session` 保留 SQLite FIFO，在上一回合终态后提交下一回合。 | LiliaCode |
| AgentKit 无失败重试 API | 沿用读取 `retryContext`、重建请求并创建新 turn id。 | LiliaCode |
| AgentKit 无标题生成 | 通过 `lilia.agent/title@1` Kernel Job 更新 `AgentSession.title`。 | LiliaCode |
| AgentKit 无 automation 关联 | 在请求/队列行保留 correlation，终态时完成 automation 节点。 | LiliaCode |
| AgentPluginRegistrar 尚未覆盖全部产品工具 | `update_project_architecture` 已有 `PluginBuilder` 执行边界（`lilia.agent.project-architecture@1`），插件只授权、不落图；确认落图仍在 `apps/desktop/src/application/agent_architecture.rs`。产品工具并没有全部改成 `AgentPluginRegistrar`。 | LiliaCode + Mutsuki |
| 分叉绑定失败留下的目标 session | 分叉已拆成两步：先 AgentKit fork 创建目标 session，再 `bind_forked_task_session`。绑定失败时任务仍指向原 session。绑定失败留下的目标 session 尚未清理。 | LiliaCode |
| 回合内 compaction 与换会话压缩不是同一条路径 | `LiliaCompact` 换会话并更换 task 绑定；AgentKit `compaction_service` 只压缩当次模型输入、不替换 durable transcript。产品 profile 不打开 `compaction_service`。压缩提交被取消或重启打断时，任务仍绑定压缩前的 session。 | LiliaCode |

## 拆除顺序上的正确性约束

队列 ack 用 `claim_token` 保证所有权：重启后的 `prepare_recovery` 会换新 token，陈旧 worker 不能确认已被新进程重投的回合。当前代码把 worker 观测到的 AgentKit `SessionVersion` 写入 `claim_epoch = sv:{n}`，并在同一进程 finish ack 校验 token+version；但恢复后的 version 仍来自 projection，AgentKit 尚未为 approval/interaction/restart 提供统一 SessionVersion。不得在这项上游能力落地前同时删掉 token 与 epoch。

## 验证记录

`cargo xtask verify` 在抓帧修复之前已经退出码 0。这次只重跑了与抓帧选择直接相关的 `lilia-xtask` 测试，没有再跑整库 verify。

`cargo xtask agent-debug` 退出码 1。产物在 `agent-debug-runs/lilia-agent-debug-1791377960081`。桌面已启动并挂载 feature。设置页截图（含 `appearance-sidebar-unified.png`）是已渲染的客户区，不再是均匀黑场。更早两次黑场（`lilia-agent-debug-1791374992426`、`lilia-agent-debug-1791376973308`）是因为抓帧选中了比主窗口更大的 `NanaWindowShadow`，`PrintWindow` 对这个没有重定向位图的阴影窗得到不透明黑帧。门禁这次停在用量趋势图：指针移到设置页签后提示仍在，code 为 `quota_tooltip_not_cleared`。同一次运行没有走到陈旧 claim、重启后的 SessionVersion、审批版本或增量事件游标。

本文只同步当前路径、接线和未完成缺口，不宣称 Issue #69 已验收或关闭。
