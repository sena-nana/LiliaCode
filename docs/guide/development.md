# 开发启动

LiliaCode 当前仓库、包名、协议名和本地配置路径仍沿用 `lilia` 命名，以避免破坏既有协议和持久化路径。

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
│               ├── todos.rs    # TodoWrite / todo_list 事件拦截 -> TaskTodo upsert
│               ├── plugins.rs  # Claude skills / plugins / MCP 与 Codex MCP 发现
│               └── lib.rs      # chat / settings / project / plugin IPC
└── packages/
    └── contracts/              # 跨端共享 TS 类型与 timeline display 规则
```

## 本地运行

```bash
# 1. 首次安装依赖
yarn install

# 2. 仅启动 Vite 前端
yarn dev

# 3. 启动 Tauri 桌面端，需要本地有 Rust 工具链和 WebView2
yarn tauri:dev

# 4. 类型检查、单测、Rust 编译检查、契约包检查
yarn verify
```

## 文档站

```bash
# 启动 VitePress 文档站
yarn docs:dev

# 构建 GitHub Pages 静态产物
yarn docs:build

# 本地预览构建产物
yarn docs:preview
```

GitHub Pages 部署由仓库中的 Actions workflow 自动完成。推送到 `main` 后，站点会构建并发布到 `https://sena-nana.github.io/LiliaCode/`。

## CI/CD

GitHub Actions 会在 pull request 到 `main`、推送到 `main` 或手动触发时运行 CI。CI 会执行 `yarn verify`，并单独构建文档站，确保桌面测试、前端构建、Tauri Rust 检查、contracts 类型检查和文档构建都通过。

推送到 `main` 后，文档站会继续由 Pages workflow 自动发布。发布 Windows 桌面安装包时推送 `v*` tag：

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

Release workflow 会先运行 `yarn verify`，再构建 Windows Tauri 安装包，并上传到 draft GitHub Release。当前发布包暂不包含代码签名、macOS 公证、Linux/macOS 安装包或自动更新。

## 图标

Tauri 图标的设计稿是 `apps/desktop/src-tauri/icons/icon.svg`，其中 PNG 嵌入在 SVG 容器内。要重新生成全套 PNG 或 ICO 时运行：

```bash
pwsh -File scripts/generate-icon.ps1
```

如需 macOS `.icns` 或全套尺寸，运行：

```bash
yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png
```
