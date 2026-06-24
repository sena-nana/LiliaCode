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
- `summary.json`
- `tauri-driver.log`

若自动准备失败且最终仍缺少 `tauri-driver`、EdgeDriver 或 debug app binary，脚本会写 `summary.json` 并以 blocked 状态退出。

## 大型改动标准

涉及 UI 主路径、Agent runtime、Tauri command、持久化、权限、构建配置或跨端契约的大型改动，最终确认必须包含：

- `yarn verify:agent-debug` 结果，或无法运行的具体 blocker。
- 至少一张调试截图路径。
- 失败时的 `summary.json` / `logs.json` 摘要。
- 若跳过该验证，必须说明剩余风险。
