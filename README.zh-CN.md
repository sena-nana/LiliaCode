<!-- 若要更换主界面截图，保持文件名 .github/assets/main-window.png 以避免改动 README -->

> [English](README.md) | 简体中文 | [网页版文档](https://sena-nana.github.io/LiliaCode/)

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

以下按当前真实接入面记录。只有已经能作为用户功能使用的项目标记为完成；部分接入和未接入项目均保持未完成。最近核对时间：2026-06-04。

### 共通 Agent 能力

- [x] 权限模式：按执行风险选择完全访问、询问、只读等执行范围，并映射到 Claude / Codex 的运行参数。
- [x] Todo 展示：镜像 Claude `TodoWrite` 和 Codex `todo_list`，展示 Agent 当前任务清单和执行进度。
- [x] 过程时间线：区分并展示 Agent 的思考、命令、工具调用、文件变更、计划和最终回复。
- [x] 关键节点跳转：在滚动条中高亮关键节点，并支持快速跳转。
- [x] 非打断交互切换：权限请求、Agent 提问和计划确认可以进入待处理区，不抢占输入框。
- [x] 引导队列：用户引导 Todo 可创建、排队、串行发送，并在运行中恢复队列状态。
- [x] MCP 基础接入：Claude stdio MCP 可由 Lilia 管理并注入运行时；Codex 可读取并管理 `~/.codex/config.toml` 中的 stdio MCP server。
- [x] 统一交互协议：跨后端统一计划确认、工具确认和 Agent 提问。
- [x] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文，也支持粘贴/拖入附件。
- [ ] 智能模型选择：还没有根据请求类型自动选择模型级别和思考强度。
- [ ] 斜杠命令：还没有支持后端原生命令和项目自定义命令。

### Claude Code 接入

- [x] Claude 对话：通过 Claude Agent SDK `query()` 发起新 turn，并保存 SDK `session_id` 用于同任务继续会话。
- [x] Claude Plan：镜像 `ExitPlanMode`，通过统一 AskUser 完成同意、取消和修订请求，确认后恢复执行阶段权限模式。
- [x] Claude Skills：管理用户级和项目级 Skills，并把启用列表传给 SDK。
- [x] Claude 工具展示：归一化 Bash、Read / Write / Edit / MultiEdit、Glob / Grep、NotebookEdit、WebSearch / WebFetch、TodoWrite、Task / Agent、ExitPlanMode 等常用工具。
- [ ] Claude MCP 管理（部分接入）：界面可增删改启停 stdio MCP server；HTTP / SSE、OAuth、elicitation、tool policy 和 SDK instance MCP 尚未接入。
- [ ] Claude Plugins（部分接入）：可发现和启停用户级本地 plugin，并把启用的 plugin path 传给 SDK；安装、更新、项目级 / marketplace 作用域管理尚未接入。
- [ ] Claude Hooks（部分接入）：运行时注册了少量 SDK hook，并能显示部分 hook lifecycle 事件；还没有 Hooks 配置管理和执行结果面板。
- [ ] Claude Subagents（部分接入）：可显示 Task / Agent 调用、任务进度和通知；还没有 subagent 定义、列表管理或主动调度 UI。

### Codex 接入

- [x] Codex 对话：通过 Codex app-server 启动 / 恢复 thread，并按 task 保存运行时状态。
- [x] Codex 过程展示：展示 Codex 的思考、命令、文件变更、搜索、计划和最终回复。
- [x] Codex 环境检查：提示 Codex CLI、app-server、API 和连接状态是否可用。
- [x] Codex Plan：启用 app-server experimental API，读取 `collaborationMode/list` 的 plan preset，并在 `turn/start` 传入 `collaborationMode`；计划确认后显式回到 default mode 执行。
- [x] Codex 审批桥接：命令和文件变更审批进入统一工具确认，支持 `additionalPermissions` / `availableDecisions`，并可由 Lilia 执行用户编辑后的 Codex 命令再回灌结果。
- [x] Codex MCP 管理：界面可查看、增删改启停用户级 `~/.codex/config.toml` stdio MCP server；HTTP / OAuth / 未知 transport 只读展示。
- [x] Codex 配置档案：支持全局 / 项目级 profile、reasoning effort、runtime workspace roots、受控 permissions，并通过 sticky `thread/settings/update` 传给 app-server。
- [ ] Codex 专项工作流：还没有内置代码审查、修复建议、批量改动等工作流。
- [ ] 内置浏览器交互：还没有通过 IAB 与用户互动或调试代码。

### LiliaCode 特色功能

- [ ] 项目级管理（部分接入）：支持本地项目、Git clone、项目排序 / 置顶 / 移除和会话数量统计；项目级进度、数据和成本视图尚未接入。
- [x] 会话任务化：会话以 Task 持久化，支持草稿提升、项目内会话、孤儿会话、归档、置顶和排序。
- [ ] 任务树（部分接入）：数据层已有 `parent_id` 和 `depends_on`，但还没有完整父子树、依赖视图和阻塞管理 UI。
- [ ] 插件系统（部分接入）：已有 Claude Skills / Plugins / MCP 和 Codex MCP 的扩展管理与运行时注入；通用插件系统和可选择的行为插件尚未成型。
- [ ] Memory（部分接入）：已有项目 Memory 页入口和扩展宿主的上下文候选能力；用户级 / 项目级记忆存储、检索和自动注入尚未形成用户功能。
- [ ] Roadmap / Milestone（部分接入）：已有项目 Roadmap 页入口和文档模板；跨周、跨版本的工程进展数据视图尚未接入。
- [ ] 自动编排：还没有根据任务状态、依赖关系和用户策略调度多个 Agent。
- [ ] 辅助 Agent：还没有在会话中运行低成本 Agent 来监督和辅助主 Agent。
- [ ] 接入 MutsukiCore：还没有远程运行任务和手机端访问能力。

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
│               ├── plugins.rs  # Claude skills / plugins / MCP 与 Codex MCP 管理
│               └── lib.rs      # chat / settings / project / plugin IPC
└── packages/
    └── contracts/              # 跨端共享 TS 类型与 timeline display 规则
```

## 早期开发

LiliaCode 通过 Corepack 使用 Yarn 4.14.1。先启用 Corepack，再从仓库根目录通过根 `yarn ...` 脚本运行贡献命令。`npm`、`pnpm`、全局 Yarn 1.x 和直接进入 workspace 运行脚本都会被检查拦住，不作为贡献路径支持。

```bash
# 1) 启用 Corepack 并激活仓库要求的 Yarn 版本
corepack enable
corepack prepare yarn@4.14.1 --activate

# 2) 安装依赖（首次）
yarn install

# 3) 仅启动 Vite 前端
yarn dev

# 4) 启动 Tauri 桌面端（需要本地有 Rust 工具链 + WebView2）
yarn tauri:dev

# 5) 运行类型检查 / 单测 / Rust 编译检查 / 契约包检查
yarn verify

# 6) 启动、构建或预览文档站
yarn docs:dev
yarn docs:build
yarn docs:preview
```

如果启用 Corepack 后 `yarn --version` 仍显示 `1.x`，请显式通过 Corepack 运行命令，例如 `corepack yarn install` 和 `corepack yarn dev`。仓库脚本和 workspace 脚本都会执行同一个包管理器检查，让贡献者统一走 Corepack 管理的 Yarn 路径。

Tauri 图标的设计稿是 [apps/desktop/src-tauri/icons/icon.svg](apps/desktop/src-tauri/icons/icon.svg)（PNG 嵌入式 SVG 容器）。要重新生成全套 PNG / ICO 时跑 `yarn icons:generate`。如需 macOS `.icns` 或全套尺寸：`yarn icons:tauri`。

## 感谢

- Codex 为界面设计和交互整理提供了重要参考；LiliaCode 的用户交互在这些思考基础上继续迭代。
