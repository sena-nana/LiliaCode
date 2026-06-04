# 功能状态

以下按最终产品能力列出。已勾选项目表示当前已经能作为用户功能使用，未勾选项目表示目标能力尚未完整补齐。

## 共通 Agent 能力

- [x] 权限模式：按执行风险选择完全访问、询问、只读等执行范围。
- [x] Todo 展示：展示 Agent 当前任务清单和执行进度。
- [x] 过程时间线：区分并展示 Agent 的思考、命令、工具调用、文件变更和回复。
- [x] 关键节点跳转：在滚动条中高亮关键节点，并支持快速跳转。
- [x] 非打断交互切换：将权限请求、Agent 提问和计划确认收进待处理区，不抢占输入框。
- [x] 引导功能：提供优先级操作队列，让用户消息和插件行为进入统一引导队列。
- [x] MCP 基础接入：支持从 Agent 配置中发现并接入 MCP server。
- [x] 统一交互协议：跨后端统一计划确认、工具确认和 Agent 提问。
- [ ] 智能模型选择：根据请求类型自动选择模型级别和思考强度。
- [x] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文。
- [ ] 斜杠命令：支持后端原生命令和项目自定义命令。

## Claude Code 接入

- [x] Claude 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Claude Skills：管理用户级和项目级 Claude Skills。
- [x] Claude MCP 管理：在界面中增删改外部 Claude MCP server。
- [ ] Claude Plugins：完整管理 Claude Plugin 的安装、启停、更新和作用域。
- [ ] Claude Hooks：管理 Claude Code Hooks，并展示执行结果。
- [ ] Claude Subagents：支持 Claude Code Subagents 或自定义 Agent 的展示与调度。

## Codex 接入

- [x] Codex 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Codex 过程展示：展示 Codex 的思考、命令、文件变更、搜索和最终回复。
- [x] Codex 环境检查：提示 Codex CLI、API 和连接状态是否可用。
- [x] Codex MCP 读取：沿用 Codex 配置接入 MCP server。
- [ ] Codex MCP 管理：在 LiliaCode 内增删改 Codex MCP server。
- [ ] Codex 配置档案：支持 profiles、沙箱、审批预设和项目级配置。
- [ ] Codex 专项工作流：支持代码审查、修复建议和批量改动等常用流程。
- [ ] 内置浏览器交互：通过 IAB 与用户互动或调试代码。

## LiliaCode 特色功能

- [ ] 项目级管理：管理本地项目和 GitHub clone 项目，查看项目级进度、数据和成本。
- [ ] 会话任务化：把对话作为 Task 管理，从而允许项目级调度。
- [ ] 任务树：完整管理父子任务、任务依赖和阻塞关系。
- [ ] 自动编排：根据任务状态、依赖关系和用户策略调度多个 Agent。
- [ ] 插件系统：将会改变 Agent 行为的能力做成可选择开启的插件。
- [ ] Memory：保存用户级和项目级记忆，并在合适时机辅助 Agent 使用。
- [ ] Roadmap / Milestone：用路线图和里程碑展示跨周、跨版本的工程进展。
- [ ] 辅助 Agent：在会话中运行低成本 Agent，实时监督和辅助主 Agent。
- [ ] 接入 MutsukiCore：支持远程运行任务和手机端访问。
