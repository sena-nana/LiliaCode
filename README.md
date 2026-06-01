<!-- 若要更换主界面截图，保持文件名 .github/assets/main-window.png 以避免改动 README -->

> **开发状态声明**
>
> Lilia 仍处于快速变更阶段；基本功能尚未完整补完；本地数据库结构可能随新功能调整，数据可能随时被清空或迁移。不建议在重度生产场景中依赖它保存唯一副本。

<p align="center">
  <img src="./apps/desktop/src-tauri/icons/icon.png" width="128" alt="Lilia logo" />
</p>

<h1 align="center">Lilia</h1>

<p align="center">
  <a href="https://qm.qq.com/q/WViyGEq8oA">
    <img alt="Static Badge" src="https://img.shields.io/badge/Lilia%E3%81%AE%E5%B0%8F%E7%AA%9D-289582454-blue">
  </a>
</p>

<p align="center"><em>Lilia（莉莉娅）—— 名字来自作者的原创角色，希望这个工具像她一样靠谱、温和、能一直陪着写代码。</em></p>

<p align="center"><strong>陪你把一个工程从想法养到成熟的 Agent 桌面客户端。</strong></p>

<p align="center">它记得你的项目、跟踪 agent 正在做什么、知道哪个会话在等哪个会话，并把工程过程整理成可继续推进的本地状态。</p>

<p align="center">
  <img src="./.github/assets/main-window.png" alt="Lilia 主界面" />
</p>

---

## Lilia 有什么不同

Lilia 不是把 Claude Code 或 Codex 包进一个聊天窗口，而是给它们外面加一层工程工作台。每次对话都落到可恢复、可整理、可继续调度的本地结构里。

也因此 Lilia 不会做对话历史兼容，因为它们在存储方式上和 Lilia 的会话是不一致的。

Lilia 将每条会话看作是一条任务，也就是说，通过开启功能标记，Lilia 会展现出与其他客户端不同的行为，例如会话内分叉，自动编排等。Lilia 更关注的是整个项目如何运作，而非单个会话。

## 功能列表

以下按最终产品能力列出；勾选表示当前已经能作为用户功能使用，未勾选表示目标能力尚未完整补齐。

### Claude 接入

- [x] Claude 对话：在 Lilia 中发起新对话并继续历史会话。
- [x] 权限模式：支持完全访问、询问、只读三种执行方式。
- [x] 常用交互：支持计划确认、工具确认和 Agent 提问。
- [x] Todo 展示：展示 Agent 当前的任务清单和执行进度。
- [ ] 智能模型选择：根据请求类型自动使用不同级别和思考强度的 Claude 模型。
- [ ] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文。
- [ ] 斜杠命令：支持 Claude Code 原生命令和项目自定义命令。
- [x] Skills 支持：管理用户级和项目级 Claude Skills。
- [x] MCP 支持：在界面中管理 Claude MCP server。
- [ ] Plugin 支持：完整管理 Claude Plugin 的安装、启停、更新和作用域。
- [ ] Hooks 支持：管理 Claude Code Hooks，并展示执行结果。
- [ ] 子代理：支持 Claude Code Subagents / 自定义 Agent 的展示与调度。
- [ ] 团队模式：在界面中并行查看多个 Agent 的分工和进度。

### Codex 接入

- [x] Codex 对话：在 Lilia 中发起新对话并继续历史会话。
- [x] 权限模式：按执行风险选择不同权限范围。
- [x] 过程展示：展示 Codex 的思考、命令、文件变更、搜索和最终回复。
- [x] 环境检查：提示 Codex CLI、API 和连接状态是否可用。
- [ ] 智能模型选择：根据请求类型自动使用不同级别和思考强度的 Codex 模型。
- [ ] 常用交互：支持计划确认、工具确认和 Agent 提问。
- [ ] 文件上下文：支持通过 `@` 提及文件、目录和图片等上下文。
- [ ] 斜杠命令：支持 Codex 原生命令和项目自定义命令。
- [x] MCP 支持：沿用 Codex 配置接入 MCP server。
- [ ] MCP 可视化管理：在 Lilia 内增删改 Codex MCP server。
- [ ] 配置档案：支持 Codex profiles、沙箱 / 审批预设和项目级配置。
- [ ] 专项工作流：支持代码审查、修复建议和批量改动等 Codex 常用流程。
- [ ] 内置浏览器交互：通过 IAB 与用户互动或调试代码。

### 特色功能

- [ ] 项目级管理：管理本地项目和 GitHub clone 项目，查看项目级进度、数据和成本。
- [ ] 会话任务化：把对话作为 Task 管理，从而允许项目级调度。
- [ ] 任务树：完整管理父子任务、任务依赖和阻塞关系。
- [x] 过程时间线：用时间线区分每个 Agent 行为过程。
- [x] 关键节点跳转：在滚动条中高亮关键节点，并支持快速跳转。
- [x] 非打断交互切换：可选地把权限请求、Agent 提问、计划确认收进待处理区，不抢占输入框。
- [x] 引导功能：提供优先级操作队列，让用户消息和插件行为进入统一引导队列。
- [ ] 插件系统：将会改变 Agent 行为的能力做成可选择开启的插件。
- [ ] Memory：保存用户级 / 项目级记忆，并在合适时机辅助 Agent 使用。
- [ ] Roadmap / Milestone：用路线图和里程碑展示跨周、跨版本的工程进展。
- [ ] 辅助 Agent：在会话中运行低成本 Agent，实时监督和辅助主 Agent。
- [ ] 接入 MutsukiCore：支持远程运行任务和手机端访问。


## 项目结构

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
- 在界面设计上，CodeX 给予我很多思考，Lilia 的用户交互是在 CodeX 的设计基础上制作的
