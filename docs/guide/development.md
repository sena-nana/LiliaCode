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
│       │   ├── styles/         # 主题令牌、标准组件样式、壳层样式和按需页面样式
│       │   ├── router.ts
│       │   └── mainBootstrap.ts
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

本仓库通过 Corepack 使用 Yarn 4.14.1。先启用 Corepack，再从仓库根目录通过根 `yarn ...` 脚本运行贡献命令。`npm`、`pnpm`、全局 Yarn 1.x 和直接进入 workspace 运行脚本都会被检查拦住，不作为贡献路径支持。

```bash
# 1. 启用 Corepack 并激活仓库要求的 Yarn 版本
corepack enable
corepack prepare yarn@4.14.1 --activate

# 2. 首次安装依赖
yarn install

# 3. 仅启动 Vite 前端
yarn dev

# 4. 启动 Tauri 桌面端，需要本地有 Rust 工具链和 WebView2
yarn tauri:dev

# 5. 运行类型检查、单测、Rust 编译检查、契约包检查
yarn verify
```

如果启用 Corepack 后 `yarn --version` 仍显示 `1.x`，请显式通过 Corepack 运行命令，例如 `corepack yarn install` 和 `corepack yarn dev`。仓库脚本和 workspace 脚本都会执行同一个包管理器检查，让贡献者统一走 Corepack 管理的 Yarn 路径。

`yarn dev` 是普通浏览器预览模式，Vite 会为 `@tauri-apps/api/*` 接入内存态轻量 mock，让页面可以在没有 Tauri shell 的情况下浏览基础项目、对话、设置和插件页面。`yarn tauri:dev`、`yarn build`、`yarn tauri:build` 不启用这套 mock，仍然通过现有 Tauri command 与 Rust/SQLite 后端通信。

## 文档站

```bash
# 启动 VitePress 文档站
yarn docs:dev

# 构建 GitHub Pages 静态产物
yarn docs:build

# 本地预览构建产物
yarn docs:preview
```

文档命令也需要从仓库根目录通过根 `yarn ...` 脚本运行。

GitHub Pages 部署由仓库中的 Actions workflow 自动完成。推送到 `main` 后，站点会构建并发布到 `https://sena-nana.github.io/LiliaCode/`。

## CI/CD

GitHub Actions 会在 pull request 到 `main`、推送到 `main` 或手动触发时运行 CI。CI 会执行 `yarn verify`，并单独构建文档站，确保桌面测试、前端构建、Tauri Rust 检查、contracts 类型检查和文档构建都通过。

推送到 `main` 后，文档站会继续由 Pages workflow 自动发布。发布 Windows 桌面安装包前，先同步并检查四处版本号：根 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml` 和 `apps/desktop/src-tauri/tauri.conf.json`。版本号必须与发布 tag 去掉 `v` 后一致。

```bash
yarn release:check --tag vX.Y.Z
```

检查通过后推送 `v*` tag：

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

Release workflow 会先运行 `yarn verify` 和 `yarn release:check --tag <tag>`，再构建 Windows Tauri 安装包，并上传到 draft GitHub Release。安装包命名按 `LiliaCode_<version>_x64-setup.*` 检查；release draft 会附带首发检查清单和自动生成的变更记录。

正式发布前必须在 Windows 环境下载 draft Release 里的安装包，确认安装、启动主窗口和卸载流程可完成。当前发布包没有代码签名，Windows SmartScreen 或安全软件警告属于预期风险；当前也不包含 macOS 公证、Linux/macOS 安装包或 Tauri updater 自动更新。首发阶段升级方式是手动下载并安装新版 Windows 安装包。

GitHub Release 正文可从 `docs/github/release-template.md` 复制后补全。

## 图标

Tauri 图标的设计稿是 `apps/desktop/src-tauri/icons/icon.svg`，其中 PNG 嵌入在 SVG 容器内。要重新生成全套 PNG 或 ICO 时运行：

```bash
yarn icons:generate
```

如需 macOS `.icns` 或全套尺寸，运行：

```bash
yarn icons:tauri
```
