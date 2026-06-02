<!-- 若要更换主界面截图，保持文件名 .github/assets/main-window.png 以避免改动 README -->

> **开发状态声明**
>
> LiliaCode 仍处于快速变更阶段；基本功能尚未完整补完；本地数据库结构可能随新功能调整，数据可能随时被清空或迁移。不建议在重度生产场景中依赖它保存唯一副本。

<p align="center">
  <img src="./apps/desktop/src-tauri/icons/icon.png" width="128" alt="LiliaCode logo" />
</p>

<h1 align="center">LiliaCode</h1>

<p align="center">
  <a href="https://qm.qq.com/q/WViyGEq8oA">
    <img alt="LiliaCode QQ 群" src="https://img.shields.io/badge/LiliaCode-289582454-blue">
  </a>
</p>

<p align="center"><strong>面向代码工程的 Agent 协同桌面客户端。</strong></p>

<p align="center">LiliaCode 将 Claude Code 与 Codex 的工作过程整理为可恢复、可追踪、可调度的本地任务状态，帮助开发者管理项目里的会话、上下文、待办和执行过程。</p>

<p align="center">
  <img src="./.github/assets/main-window.png" alt="LiliaCode 主界面" />
</p>

---

## 产品定位

LiliaCode 是 Lilia 系列中的代码工程工作台。它不是把 Claude Code 或 Codex 包进一个聊天窗口，而是在 Agent 执行层之外提供项目、任务、会话、权限和过程信息的桌面级组织层。

它面向需要长期推进工程项目的开发者：每条会话都可以被视作可管理的任务，Agent 的执行过程和待处理交互会沉淀为本地状态，并为后续任务树、自动编排和多 Agent 协同提供基础。

## Lilia 系列

Lilia 是面向高 Agent 协同的工具链应用系列。系列目标是把不同 Agent、执行环境和工程工作流接入同一套可观察、可调度、可恢复的本地工作台。

LiliaCode 聚焦代码工程场景；同系列应用可以继续围绕其他高协同 Agent 工作流扩展，并共享项目状态、任务化会话、插件化能力和人机协作边界等基础理念。

## 核心差异

- 任务化会话：将对话作为任务管理，而不是只保存聊天记录。
- 本地工程状态：记录项目、会话、待办、过程和关键交互，便于恢复和继续推进。
- 过程可观察：用时间线呈现 Agent 的思考、工具调用、命令执行、文件变更和最终回复。
- 非打断交互：权限请求、计划确认和 Agent 提问可以进入待处理区，减少对输入流的打断。
- 面向协同调度：为任务树、依赖关系、自动编排和辅助 Agent 留出统一结构。

由于会话存储模型与上游 CLI / SDK 的历史记录不同，LiliaCode 不以兼容原始对话历史为目标；它优先维护自己的可恢复任务结构。

## 功能状态

以下按最终产品能力列出；勾选表示当前已经能作为用户功能使用，未勾选表示目标能力尚未完整补齐。

### 共通 Agent 能力

- [x] 权限模式：按执行风险选择完全访问、询问、只读等执行范围。
- [x] Todo 展示：展示 Agent 当前任务清单和执行进度。
- [x] 过程时间线：区分并展示 Agent 的思考、命令、工具调用、文件变更和回复。
- [x] 关键节点跳转：在滚动条中高亮关键节点，并支持快速跳转。
- [x] 非打断交互切换：将权限请求、Agent 提问和计划确认收进待处理区，不抢占输入框。
- [x] 引导功能：提供优先级操作队列，让用户消息和插件行为进入统一引导队列。
- [x] MCP 基础接入：支持从 Agent 配置中发现并接入 MCP server。
- [ ] 统一交互协议：跨后端统一计划确认、工具确认和 Agent 提问。
- [ ] 智能模型选择：根据请求类型自动选择模型级别和思考强度。
- [x] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文。
- [ ] 斜杠命令：支持后端原生命令和项目自定义命令。

### Claude Code 接入

- [x] Claude 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Claude Skills：管理用户级和项目级 Claude Skills。
- [x] Claude MCP 管理：在界面中增删改外部 Claude MCP server。
- [ ] Claude Plugins：完整管理 Claude Plugin 的安装、启停、更新和作用域。
- [ ] Claude Hooks：管理 Claude Code Hooks，并展示执行结果。
- [ ] Claude Subagents：支持 Claude Code Subagents / 自定义 Agent 的展示与调度。

### Codex 接入

- [x] Codex 对话：在 LiliaCode 中发起新对话并继续历史会话。
- [x] Codex 过程展示：展示 Codex 的思考、命令、文件变更、搜索和最终回复。
- [x] Codex 环境检查：提示 Codex CLI、API 和连接状态是否可用。
- [x] Codex MCP 读取：沿用 Codex 配置接入 MCP server。
- [ ] Codex MCP 管理：在 LiliaCode 内增删改 Codex MCP server。
- [ ] Codex 配置档案：支持 profiles、沙箱 / 审批预设和项目级配置。
- [ ] Codex 专项工作流：支持代码审查、修复建议和批量改动等常用流程。
- [ ] 内置浏览器交互：通过 IAB 与用户互动或调试代码。

### LiliaCode 特色功能

- [ ] 项目级管理：管理本地项目和 GitHub clone 项目，查看项目级进度、数据和成本。
- [ ] 会话任务化：把对话作为 Task 管理，从而允许项目级调度。
- [ ] 任务树：完整管理父子任务、任务依赖和阻塞关系。
- [ ] 自动编排：根据任务状态、依赖关系和用户策略调度多个 Agent。
- [ ] 插件系统：将会改变 Agent 行为的能力做成可选择开启的插件。
- [ ] Memory：保存用户级 / 项目级记忆，并在合适时机辅助 Agent 使用。
- [ ] Roadmap / Milestone：用路线图和里程碑展示跨周、跨版本的工程进展。
- [ ] 辅助 Agent：在会话中运行低成本 Agent，实时监督和辅助主 Agent。
- [ ] 接入 MutsukiCore：支持远程运行任务和手机端访问。

## 项目结构

> 当前仓库、包名、协议名和本地配置路径仍沿用 `lilia` 命名，以避免破坏既有协议和持久化路径。

```text
Lilia/
├── apps/
│   └── desktop/                # 主应用：Vue 3 + Tauri 2
│       ├── src/
│       │   ├── layouts/        # AppShell / SecondaryPanel / TitleBar
│       │   ├── components/     # ViewTabs / TodoFloat / ChatComposer 等
│       │   ├── pages/          # project/ProjectShell / TaskDetail / Settings
│       │   ├── services/       # projectsStore / tasksStore / todos / chat
│       │   ├── router.ts
│       │   └── styles.css
│       └── src-tauri/          # Tauri 2 Rust 端
│           └── src/
│               ├── store.rs    # lilia-store：SQLite + r2d2 + 迁移
│               ├── todos.rs    # TodoWrite / todo_list 事件拦截 → TaskTodo upsert
│               ├── plugins.rs  # Claude skills / plugins / MCP 与 Codex MCP 发现
│               └── lib.rs      # chat / settings / project / plugin IPC
└── packages/
    └── contracts/              # 跨端共享 TS 类型与 timeline display 规则
```

## 早期开发

```bash
# 1) 安装依赖（首次）
yarn install

# 2) 仅启动 Vite 前端
yarn dev

# 3) 启动 Tauri 桌面端（需要本地有 Rust 工具链 + WebView2）
yarn tauri:dev

# 4) 类型检查 / 单测 / Rust 编译检查 / 契约包检查 一键过
yarn verify
```

Tauri 图标的设计稿是 [apps/desktop/src-tauri/icons/icon.svg](apps/desktop/src-tauri/icons/icon.svg)（PNG 嵌入式 SVG 容器）。要重新生成全套 PNG / ICO 时跑 [`scripts/generate-icon.ps1`](scripts/generate-icon.ps1)：`pwsh -File scripts/generate-icon.ps1`。如需 macOS `.icns` 或全套尺寸：`yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png`。

## 感谢

- Codex 为界面设计和交互整理提供了重要参考；LiliaCode 的用户交互在这些思考基础上继续迭代。
