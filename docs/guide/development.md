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

`yarn tauri:build:no-bundle` 会执行发布级别编译但跳过安装包生成，适合发布前快速验证本机打包链路。

# 5. 运行类型检查、单测、Rust 编译检查、契约包检查
yarn verify
```

如果启用 Corepack 后 `yarn --version` 仍显示 `1.x`，请显式通过 Corepack 运行命令，例如 `corepack yarn install` 和 `corepack yarn dev`。仓库脚本和 workspace 脚本都会执行同一个包管理器检查，让贡献者统一走 Corepack 管理的 Yarn 路径。

`yarn dev` 是普通浏览器预览模式，Vite 会为 `@tauri-apps/api/*` 接入内存态轻量 mock，让页面可以在没有 Tauri shell 的情况下浏览基础项目、对话、设置和插件页面。`yarn tauri:dev`、`yarn build`、`yarn tauri:build`、`yarn tauri:build:no-bundle` 不启用这套 mock，仍然通过现有 Tauri command 与 Rust/SQLite 后端通信。

`yarn tauri:install` 会先注入本机 CPU 优化参数再执行打包，再打开安装程序并尝试安装；该入口面向本机安装验证，不用于通用分发。

可通过 dry-run 校验打包参数：

```bash
TAURI_TEMPLATE_INSTALL_DRY_RUN=1 yarn tauri:install
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

文档命令也需要从仓库根目录通过根 `yarn ...` 脚本运行。

GitHub Pages 部署由仓库中的 Actions workflow 自动完成。推送到 `main` 后，站点会构建并发布到 `https://sena-nana.github.io/LiliaCode/`。

## CI/CD

GitHub Actions 会在 pull request 到 `main`、推送到 `main` 或手动触发时运行 CI。CI 会执行 `yarn verify`，并单独构建文档站，确保桌面测试、前端构建、Tauri Rust 检查、contracts 类型检查和文档构建都通过。

推送到 `main` 后，文档站会继续由 Pages workflow 自动发布。发布 Windows 桌面安装包前，先同步并检查四处版本号：根 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml` 和 `apps/desktop/src-tauri/tauri.conf.json`。版本号必须与发布 tag 去掉 `v` 后一致。

```bash
yarn release:check --tag v1.0.0-beta.1
```

检查通过后推送 `v*` tag：

```bash
git tag v1.0.0-beta.1 && git push origin v1.0.0-beta.1
```

Release workflow 会先运行 `yarn verify` 和 `yarn release:check --tag <tag>`，再构建 Windows Tauri NSIS 安装包和 updater 产物，并上传到 draft GitHub Release。安装包命名按 `LiliaCode_<version>_x64-setup.*` 检查；updater 产物必须包含 `latest.json`、`*.nsis.zip` 和 `*.nsis.zip.sig`。release draft 会附带发布检查清单和自动生成的变更记录。

`release:check` 会自动核对版本号、Tauri Windows 打包配置、updater endpoint、`bundle.createUpdaterArtifacts`、GitHub 仓库变量 `TAURI_UPDATER_PUBKEY`、NSIS CLI 安装 hook、release notes 已知限制、安装包命名预期、Windows 安装验证记录入口，以及 release workflow 是否接入安装包 smoke 和 updater 产物上传。检查通过只代表发布元数据、记录入口和 workflow gate 已准备好。

`release:smoke:windows` 可重复执行 Windows 安装包 smoke：从本地安装包或 draft Release 安装包出发，覆盖安装、启动、`liliacode <测试项目路径>` 和卸载后的 CLI 清理。CI 会在 draft Release 生成后自动运行：

```bash
yarn release:smoke:windows --tag v1.0.0-beta.1
```

本地复核已有安装包时传入安装包路径：

```bash
yarn release:smoke:windows --installer path/to/LiliaCode_1.0.0-beta.1_x64-setup.exe
```

正式发布前，仍需要在 Release 正文的 Windows 安装验证记录中写入验证人、验证日期、Windows 环境、安装包文件名和安装 / 启动 / CLI / 卸载结果。

当前发布包使用 `tauri-signing.key` 完成签名，私钥来自 `TAURI_SIGNING_PRIVATE_KEY` secret，Tauri updater 公钥来自 GitHub 仓库变量 `TAURI_UPDATER_PUBKEY`。Windows SmartScreen 或安全软件警告风险已由签名策略降低。当前不包含 macOS 公证或 Linux/macOS 安装包。Windows 桌面应用启动后会自动检查更新，用户确认后自动下载、安装并重启；用户也可以手动下载并安装新版 Windows 安装包。

GitHub Release 正文可从 `docs/github/release-template.md` 复制后补全。

## 图标

Tauri 图标的设计稿是 `apps/desktop/src-tauri/icons/icon.png`。要用 Tauri CLI 重新生成桌面 PNG 或 ICO 时运行：

```bash
yarn icons:generate
```

`icons:tauri` 保留为同一套生成入口：

```bash
yarn icons:tauri
```

