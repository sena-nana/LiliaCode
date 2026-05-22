<!-- 若要更换主界面截图，保持文件名 .github/assets/main-window.png 以避免改动 README -->

<p align="center">
  <img src="./apps/desktop/src-tauri/icons/icon.png" width="128" alt="Lilia logo" />
</p>

<h1 align="center">Lilia</h1>

<p align="center"><em>Lilia（莉莉娅）—— 名字来自作者的原创角色，希望这个工具像她一样靠谱、温和、能一直陪着写代码。</em></p>

<p align="center"><strong>陪你把一个工程从想法养到成熟的 Claude Code 桌面客户端。</strong></p>

<p align="center">它记得你的项目、跟踪 agent 正在想什么、知道哪个会话在等哪个会话、能讲清楚工程走到了哪一站。</p>

<p align="center">
  <img src="./.github/assets/main-window.png" alt="Lilia 主界面" />
</p>

---

## 为什么会有 Lilia

一个工程从想法到成熟，自然会经过这些阶段——

- **冒头**：一句"我想做个 X"在收集箱里成形（草稿对话）
- **拆解**：确定要做后开成项目，原本的草稿继续往下生根，长出子任务和前置依赖（"得先把 schema 定下来才能改 IPC"）
- **执行**：每个会话里 agent 一边干活一边把它的 Todo 摆出来，你能看见它在想什么
- **沉淀**：会话之间需要记得"上次为什么这么决定"——Memory 在用户级和项目级两层贴附
- **回望**：跨周/月的进展需要一个能讲故事的位置——Roadmap / Milestone 不画甘特图，只把节点串成时间线

Claude Code 原生擅长**单次对话**，但**整个工程的演化**没有一个能承载它的容器。Lilia 不是要替代 Claude Code，而是在 Claude Code 之上长出"项目生命周期"这一层。

> *任务图、Todo、Memory、Roadmap 都是手段。Lilia 真正在做的是——让一个工程在它走过的每一站都有人陪着。*

---

## Lilia 是如何陪你的

### 记得每个会话在工程里的位置（Session = Task）

Claude Code 原生只把对话存成扁平 JSONL；Lilia 让每个 session **1:1 绑定到一个 Task**，可设 `parentId`（子任务）与 `dependsOn`（前置任务）。你能看见"这个会话在等谁"、"这个会话拆出过哪些后续"。

任务状态机：`draft → waiting → running → blocked → done / cancelled` —— 草稿不会污染结构化视图。数据契约见 [packages/contracts/src/index.ts](packages/contracts/src/index.ts)。

### 把 agent 正在想什么摆出来（Todo as AI thinking visualization）

当 Claude SDK 内部触发 `TodoWrite` 工具时，Lilia 拦截事件、把 todos upsert 到 SQLite、推送给前端。

**重点：Todo 不是任务管理**——它是 agent 思考过程的实时镜面。它出现在 composer 顶端的 chip / 卡片里，零 todo 时不渲染，一旦 agent 开始规划就自动展开。你不再需要追着问"你下一步打算干嘛"。

实现入口：[apps/desktop/src-tauri/src/todos.rs](apps/desktop/src-tauri/src/todos.rs)。

### 让项目记得自己的偏好与决定（Memory）

跨会话的知识沉淀，**只有用户级 + 项目级两层**——刻意不做更细粒度。发送消息时自动拼成 `<memory scope="...">` 前缀注入到 prompt。允许从草稿对话保存（草稿比会话长寿）——记忆的载体可以游离于结构之外。

### 把跨周的进展讲成一个故事（Roadmap / Milestone）

项目长跨度叙事：**垂直时间线 + 进度条**，刻意不做甘特图（避免变成 PM 工具）。里程碑不能嵌套，只能并列（v0.5 → v0.9 → v1.0）——强迫聚焦。Task 可游离于 Milestone 之外，路线图视图额外提供"未归类"分组。

---

### 颗粒度金字塔：陪伴发生在每一层

```
Milestone（季度 / 月）       ← Lilia 让你能讲清楚工程走到了哪一站
  └── Task（天 / 周）         ← Lilia 让你看见会话与会话的依赖
       ├── Todo（小时）        ← Lilia 让 agent 的思考实时可见
       └── ChatMessage（分钟） ← Claude Code 原生擅长的层
横向：Memory 贴附在 Project / User，让记忆跨越会话存活
```

### 工程化兜底：双 Backend + 本地优先

- **双 Backend + 灵活路由**：同时支持 [`@anthropic-ai/claude-agent-sdk`](apps/desktop/package.json) 与 `@openai/codex-sdk`；每个 Backend 独立选择 CC-Switch 本地代理（`127.0.0.1:15721` 自动探测）或直连官方 API；composer 行可切 Backend / Model / Permission / Branch。
- **本地优先 · 自建对话存储**：所有结构化数据落在 `~/.lilia/`（支持 `LILIA_HOME` 与 `.redirect` 重定向）；SQLite + r2d2 + WAL + 迁移框架；对话主存 SQLite、JSONL 作 Claude Code 兼容镜像。实现见 [apps/desktop/src-tauri/src/store.rs](apps/desktop/src-tauri/src/store.rs)。

---

## 技术栈

- **Tauri 2**（Rust）+ **Vue 3.5** + **TypeScript 5.8** + **Vite 7**
- **Yarn 4 Workspaces**：`apps/desktop` + `packages/contracts`
- **SQLite**：rusqlite 0.32 bundled + r2d2 连接池 + WAL + `user_version` 迁移
- **双 Backend**：`@anthropic-ai/claude-agent-sdk` + `@openai/codex-sdk`
- **UI**：自绘 TitleBar / frameless window，CSS 变量驱动的暗浅双主题

## 项目结构

```
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
│               ├── todos.rs    # TodoWrite 事件拦截 → TaskTodo upsert
│               └── plugins.rs  # Claude skills / Codex MCP 发现
└── packages/
    └── contracts/              # 跨端共享 TS 类型（Project / Session / Task / Todo / Memory / Milestone）
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
