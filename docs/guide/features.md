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
- [x] 统一 Lilia 协议：界面层只暴露 Lilia 操作，用户可见工作流和运行时命令在内部按 Claude / Codex provider 分发。
- [x] 智能模型选择：在当前后端内根据工作流、计划模式、上下文规模和 provider 能力自动选择模型级别与思考强度，并允许发送前手动覆盖。
- [x] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文。
- [x] 斜杠命令：支持在输入框通过 `/` 打开命令面板，执行内置命令和 `.lilia/commands` 项目命令，并把执行结果回写到任务 timeline；还不承诺完整代理后端原生命令。

## Claude Code 接入

- [x] Claude 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Claude 提示建议：展示 Claude 原生建议的后续用户提示。
- [x] Claude 历史导入：搜索、预览并导入本地 Claude 历史会话。
- [x] Claude Skills：管理用户级和项目级 Claude Skills。
- [x] Claude MCP 管理：在界面中增删改外部 Claude MCP server。
- [x] Claude Lilia 协议：支持审查、修复建议、批量应用、运行时命令会话分叉和 Goal / 诊断类本地落点。
- [x] Claude 思考强度：发送时可自动或手动设置 reasoning effort，并映射为 Claude Agent SDK `effort` / adaptive thinking。
- [ ] Claude Plugins：完整管理 Claude Plugin 的安装、启停、更新和作用域。
- [ ] Claude Hooks：管理 Claude Code Hooks，并展示执行结果。
- [ ] Claude Subagents：支持 Claude Code Subagents 或自定义 Agent 的展示与调度。

## Codex 接入

- [x] Codex 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Codex 过程展示：展示 Codex 的思考、命令、文件变更、搜索和最终回复。
- [x] Codex 环境检查：提示 Codex CLI、API 和连接状态是否可用。
- [x] Codex MCP 管理：读取并增删改启停用户级 stdio Codex MCP server，HTTP / OAuth / 未知 transport 只读展示。
- [x] Codex 配置档案：支持 profiles、reasoning effort、runtime workspace roots、受控 permissions 和项目级默认。
- [x] Codex 思考强度：发送时可自动或手动设置 reasoning effort，计划模式使用本轮已选 effort。
- [x] Codex Lilia 适配：支持审查、修复建议、批量应用、压缩、Goal、memory、配置诊断和后台终端清理，会话分叉通过运行时命令处理。
- [x] 内置浏览器交互：Codex 可打开和导航 IAB 窗口，采集页面标题 / URL / 截图元数据，并把结果送回运行中的 turn 或作为消息附件；截图采集目前以 Windows 为主。

## LiliaCode 特色功能

- [x] 项目级管理：管理本地项目和 GitHub clone 项目，项目总览可查看任务状态分布、最近活跃、进行中 / 阻塞数量、会话 / 任务统计和已知用量成本。
- [x] 会话任务化：会话以 Task 持久化，支持草稿提升、项目 / 孤儿会话、归档、置顶和排序。
- [ ] 任务树：完整管理父子任务、任务依赖和阻塞关系。
- [ ] 自动编排：根据任务状态、依赖关系和用户策略调度多个 Agent。
- [ ] 插件系统：将会改变 Agent 行为的能力做成可选择开启的插件。
- [x] Memory：支持手动保存用户级和项目级记忆，并在会话启动时按 Layer 1 基线注入；外置模型检索与机会窗口引导尚未实现。
- [ ] Roadmap / Milestone：项目路线图页可按当前项目 Task 状态聚合首发 milestone、进度、状态分布、当前重点和最近完成；持久化 Milestone / TaskMilestoneLink 数据源尚未接入。
- [ ] 辅助 Agent：在会话中运行低成本 Agent，实时监督和辅助主 Agent。
- [x] 内置 Lilia 协议：运行时只保留单一内置协议路径。
