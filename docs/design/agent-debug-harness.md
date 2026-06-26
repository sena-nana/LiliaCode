# Agent Debug Harness

## 目标

Lilia 的 Agent 调试层是开发态半白盒 UI 调试框架，不是生产功能。它把三类信号合在一起：

- `tauri-driver` / WebDriver：启动真实桌面应用并获取截图。
- `window.__liliaAgentDebug`：读取路由、可操作元素树、可见文本、前端错误和 invoke trace，并执行少量用户动作。
- `agent_debug_*` Tauri commands：保存前端动作日志，读取后端运行时摘要，清理调试缓冲。

调试层只在 `LILIA_AGENT_DEBUG=1` 与 `VITE_LILIA_AGENT_DEBUG=1` 的开发/测试环境中生效。未开启时前端不会安装 `window.__liliaAgentDebug`；后端命令返回 disabled。

## 接口

前端全局对象：

- `observe()`：返回 `AgentDebugSnapshot`，包含 route、title、viewport、activeElement、`data-agent-id` 元素树、最近前端错误和 invoke 记录。
- `act(action)`：支持 `click`、`type`、`hotkey`、`mark`。目标必须是稳定的 `data-agent-id`。
- `mark(label, data?)`：写入一条调试标记。
- `getRecentErrors()`：读取最近 console/error/unhandledrejection。

后端命令：

- `agent_debug_status`
- `agent_debug_logs`
- `agent_debug_runtime_snapshot`
- `agent_debug_record_action`
- `agent_debug_reset_state`

这些命令不改变业务数据。`reset_state` 只清空调试日志缓冲。

## 元素标识

主要可操作控件必须提供稳定 `data-agent-id`。命名用功能路径，不用 class、文本或坐标，例如：

- `sidebar.new-chat`
- `sidebar.project.<projectId>.row`
- `chat.composer.input`
- `chat.composer.send`
- `timeline.event.<eventId>.retry`
- `settings.tab.providers`

Agent 应先 `observe()` 查元素树，再根据 `data-agent-id` 操作，最后用截图确认视觉结果。

## 验证

运行：

```powershell
yarn verify:agent-debug
```

脚本会自动准备调试环境：

- 将 `%USERPROFILE%\.cargo\bin` 与 `%USERPROFILE%\.lilia\agent-debug\bin` 加入本次进程 `PATH`。
- 缺少 `tauri-driver` 时，使用 `cargo install tauri-driver` 安装。
- 检测本机 Microsoft Edge 版本；缺少匹配 EdgeDriver 时，下载并缓存到 `%USERPROFILE%\.lilia\agent-debug\bin`。可用 `LILIA_AGENT_DEBUG_TOOL_DIR` 改写缓存目录，用 `LILIA_AGENT_DEBUG_EDGE_VERSION` 指定无法自动探测时的浏览器版本。
- 缺少 debug app binary 时仍会阻塞；可先构建桌面 debug 包，或用 `LILIA_AGENT_DEBUG_APP` 指向已有 binary。

脚本会写入 `agent-debug-runs/<timestamp>/`：

- `preflight.json`
- `observe.json`
- `logs.json`
- `before.png`
- `after.png`
- `replay.json`
- `scenario-results.json`
- `summary.json`
- `tauri-driver.log`
- `dev-server.log`

`logs.json`、`replay.json`、`scenario-results.json`、`tauri-driver.log`、`dev-server.log` 和 `summary.json` 是门禁固定产物。即使自动准备失败、provider 未就绪或场景中途失败，脚本也必须写出这些文件，并在 `summary.json` 中记录对应路径。

若自动准备失败且最终仍缺少 `tauri-driver`、EdgeDriver 或 debug app binary，脚本会以 blocked 状态退出；此时 `scenario-results.json` 为空，`summary.json` 的截图引用为 `null`。

## v1.0 核心对话回归

`verify:agent-debug` 先保留原有 smoke：确认调试 API 启用、可见交互元素都有 `data-agent-id`、`mark` 可记录、缺失目标会返回可诊断错误。

smoke 之后会运行可重放场景，所有动作写入 `replay.json`，每个场景截图和结果写入 `scenario-results.json`：

- 普通发送：创建开发态新对话，输入内容，点击 `chat.composer.send`，确认到达 `chat_send_message` invoke 边界。
- 继续历史会话：在同一对话路由继续输入并发送，确认继续发送仍到达 `chat_send_message` invoke 边界。
- 任务恢复：离开当前对话后通过浏览器历史恢复原任务路由，确认 `chat.composer.input` 可重新操作。
- plan pending action：通过 Debug 侧栏 `debug.timeline.plan` 注入计划确认，使用 `chat.pending.plan.accept` / `chat.composer.plan.accept` 同意并确认时间线进入已同意状态。
- permission pending action：通过 Debug 侧栏 `debug.timeline.permission` 注入权限申请，使用 `chat.pending.tool.allow` / `chat.composer.tool.allow` 同意并确认时间线进入已同意状态。

这些场景只验证开发态 UI、调试注入和 Tauri invoke 边界，不等待真实 provider 生成回复。`VITE_LILIA_AGENT_DEBUG=1` 下任务详情会自动注册 Debug 侧栏面板，避免验证依赖用户本机设置里的 debug 开关。

如果 `chat_send_message` 到达 invoke 边界但返回 provider-not-ready 类错误，门禁会把当前场景写为 `blocked`，保留场景截图，并以退出码 2 结束。若任一场景在开始后中途失败，门禁会把当前 active scenario 补写为 `failed`，引用 `failure.png`，从而让 partial run 也能定位最后失败的场景。

## 大型改动标准

涉及 UI 主路径、Agent runtime、Tauri command、持久化、权限、构建配置或跨端契约的大型改动，最终确认必须包含：

- `yarn verify:agent-debug` 结果，或无法运行的具体 blocker。
- 至少一张调试截图路径。
- 失败时的 `summary.json` / `logs.json` 摘要。
- 若跳过该验证，必须说明剩余风险。
