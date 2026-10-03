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

下列项目仍是未闭合缺口，不能据此宣称 Issue #69 已验收：

- `claim_token`/`claim_epoch` 仍是队列 ack 的所有权保护。当前代码把 worker 观测到的 `SessionVersion` 绑定到 claim，并在同一进程的 finish ack 校验 token+version；但恢复后的 version 仍来自 projection，AgentKit 尚未为 approval/interaction/restart 提供统一的 SessionVersion，因此上游权威没有完全闭合。
- Kernel Job payload 携带提交时捕获的不可变 `claim_token`。turn、审批、交互和 compaction 在触碰 AgentKit 前及页面/终态路径都会用该 token 复核 durable claim；发现旧 token 时丢弃旧 worker 结果，不发终态或恢复错误，也不清理新 owner，让恢复后的 owner 继续处理。缺少 token 的旧 Job 会被拒绝。
- AgentKit 的 `AgentSessionCoordinator` 尚未接入 loop plugin；`PermissionRequest.version` 仍可能来自 transcript message 长度，而不是 session version。恢复审批时必须透传请求携带的 version，不能由 LiliaCode 推导。
- 事件投影已经有 `AgentEventEnvelope.sequence` 增量路径，但取消、恢复和部分审批/交互回调仍发 `TimelineChanged { cursor: None }`。
- session branch/fork 仍由 `feature-agent-session::turn_run` 通过 `AgentTurnHost` 编排，尚未全部变成 AgentKit fork 请求。
- `LiliaCompact` 是 LiliaCode 保留的显式产品工作流；它仍调用应用侧 compaction，不应描述成 AgentKit 已接管所有回合内压缩。
- 当前工作区记录的 `cargo xtask agent-debug` 在 Darwin 上因 `windows_required` 未运行，未产生 debug artifact；因此验证证据不足以关闭 Issue。

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
| 10 | 回合内上下文压缩 | `turn_run.rs` 仍把 `LiliaCompact` 交给 `AgentTurnHost::run_compaction`，最终由 `agent.rs::run_context_compaction_turn` 执行产品 compaction。 | **显式产品工作流保留**；AgentKit profile compaction 尚未替代此路径。 |
| 10b | 显式换会话压缩 | `context_compaction.rs` 生成摘要并替换 task/session binding。 | **保留**，这是产品工作流，不是 AgentKit 单回合状态。 |
| 11 | 自动续回合 | `finish_turn` 调用 feature queue 的 `ack_and_claim_next`；`auto_turn.rs` 仅作模型/档位选择。 | **保留**，属于跨回合产品编排。 |
| 12 | 标题更新 | `title_update.rs` 通过 `lilia.agent/title@1` Kernel Job 调度，启动时由 `desktop.rs` 安装 `QueuedTitleScheduler`。 | **已接 Kernel Job；不应恢复私有标题线程池**。 |
| 13 | 事件投影 | `agent_turn_host.rs` 的 observed 路径使用 `AgentEventEnvelope.sequence`；`agent.rs` 的取消、恢复及部分响应路径仍使用 `TimelineChanged { cursor: None }`。 | **部分增量化**；仍需收敛剩余整片 refresh。 |
| 14 | 工具注册 | 当前仍有 LiliaCode bespoke wire/host adapters；尚未看到 worktree、architecture、memory、todo、automation 全部通过 `AgentPluginRegistrar` 注册的实现。 | **未闭合**；继续保留适配边界，不宣称已完成插件化。 |
| 15 | 持久化 | AgentKit transcript/checkpoint 是审批与交互事实；`desktop_pending_turns` 是跨回合队列，`pending_projections` 仍是可重建的产品/UI 缓存，`agent_session_bindings` 保留 task↔session 绑定。 | **收敛中**；不得把缓存表当恢复权威。 |
| 16 | 线程与执行 | 生产启动在 `desktop.rs` 安装 `QueuedTurnExecutor`，发 `Message::RequestTurnJob`/`RequestApprovalJob`/`RequestInteractionJob`，再由 Kernel Job 调用 `DesktopTurnPort::execute_*_job`。`agent.rs` 不再 spawn 回合/审批/交互线程；未安装 executor 的测试/无 host 路径是 caller-thread fallback。 | **生产接线已完成；Job 缺少 claim token 时拒绝执行**。 |
| 17 | 会话分叉 | `crates/features/lilia-feature-agent-session/src/turn_run.rs` 仍调用 `AgentTurnHost::fork_through_turn`/`fork_session`，由应用侧 `agent_turn_host.rs` 访问 AgentKit wire；完成后再更新 task binding。 | **部分接线**；仍有 LiliaCode fork orchestration 缺口。 |

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
| 增量时间线未全覆盖 | 保留 `AgentEventEnvelope.sequence`/`subscribe_events(after_sequence)` 主路径，逐步替换 `agent.rs` 中取消、恢复、响应场景的 `cursor: None`。 | LiliaCode |
| AgentKit 无跨回合队列 | `feature-agent-session` 保留 SQLite FIFO，在上一回合终态后提交下一回合。 | LiliaCode |
| AgentKit 无失败重试 API | 沿用读取 `retryContext`、重建请求并创建新 turn id。 | LiliaCode |
| AgentKit 无标题生成 | 通过 `lilia.agent/title@1` Kernel Job 更新 `AgentSession.title`。 | LiliaCode |
| AgentKit 无 automation 关联 | 在请求/队列行保留 correlation，终态时完成 automation 节点。 | LiliaCode |
| AgentPluginRegistrar 尚未覆盖产品工具 | 盘点各 bespoke host adapter，补齐工具描述符、generation 和 target protocol 后再移除重复 dispatch。 | LiliaCode + Mutsuki |
| Fork orchestration 仍在 host | 将 branch/fork 的 session 创建、through-turn 复制和 binding 更新拆分成 AgentKit fork 请求与明确的产品 binding 步骤。 | LiliaCode + Mutsuki |
| 回合内 compaction 路径未统一 | 明确 AgentKit profile compaction 与 `LiliaCompact` 产品工作流的边界；在替代实现和恢复测试完成前保留现有 host 路径。 | LiliaCode + Mutsuki |

## 拆除顺序上的正确性约束

队列 ack 用 `claim_token` 保证所有权：重启后的 `prepare_recovery` 会换新 token，陈旧 worker 不能确认已被新进程重投的回合。当前代码把 worker 观测到的 AgentKit `SessionVersion` 写入 `claim_epoch = sv:{n}`，并在同一进程 finish ack 校验 token+version；但恢复后的 version 仍来自 projection，AgentKit 尚未为 approval/interaction/restart 提供统一 SessionVersion。不得在这项上游能力落地前同时删掉 token 与 epoch。

## 验证记录

本次验证通过临时解包的 Debian 13 GTK/GLib sysroot 完成了 desktop crate 的完整库检查；该 sysroot 位于 `/tmp`，没有写入仓库。`cargo xtask verify` 的 boundary-check 与 pin-check 通过，workspace 测试编译出 632 个 desktop 测试并运行了 632 项，其中 630 项通过；自动化 controller 测试重跑后通过，offscreen 测试仍因当前 Linux WGPU adapter 不支持 `Rgba32Float` 而失败。该失败不触及 Issue #69 改动，但因此不能把本次 workspace 门禁记为全绿。feature-agent-session 测试 55/55、lilia-agent 测试 94 通过（1 ignored），desktop `cargo check --locked -p lilia-desktop --lib` 通过。`cargo xtask agent-debug` 在当前 Linux 环境返回 `desktop_required`，没有 `agent-debug-runs/lilia-*` artifact；该 harness 要求 macOS 或 Windows 的真实 WGPU 桌面。后续验收仍需在支持的平台生成 debug artifact，并覆盖陈旧 claim ack、重启恢复、审批/交互版本和增量事件游标。

本文只同步当前路径、接线和未完成缺口，不宣称 Issue #69 已验收或关闭。
