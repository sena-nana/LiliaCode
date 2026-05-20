# Lilia

一个为了满足自己需求而做的 Claude Code 客户端。

> 表面上仍是「分项目 → 分 session 的对话模式」，但内部把每个 session 识别为一个 *任务*，
> 因此允许出现「子任务、前置任务」等本来属于 todo 软件的概念。

## 技术栈

- **Tauri 2** + **Vue 3** + **TypeScript** + **Vite**
- 包管理：**Yarn 4 Workspaces**（`apps/desktop`、`packages/contracts`）
- 视觉风格参考同父目录下的 `Momo`。

## 目录

```
Lilia/
├── apps/
│   └── desktop/                # 主应用：Vue 3 + Tauri 2
│       ├── src/
│       │   ├── layouts/        # AppShell / ActivityBar / SecondaryPanel
│       │   ├── components/     # TitleBar 等通用组件
│       │   ├── pages/          # Projects / ProjectDetail / TaskDetail / Settings
│       │   ├── data/           # 暂为 stub，将替换为真实仓库层
│       │   ├── router.ts
│       │   ├── main.ts
│       │   ├── App.vue
│       │   └── styles.css
│       ├── src-tauri/          # Tauri 2 Rust 端
│       ├── tests/              # vitest
│       ├── index.html
│       ├── vite.config.ts
│       └── package.json
└── packages/
    └── contracts/              # 跨端共享的 TS 类型（Project / Session / Task）
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

### Tauri 图标

应用图标由 [`scripts/generate-icon.ps1`](scripts/generate-icon.ps1) 用 PowerShell + GDI+
程序化绘制：浅蓝色 6 臂雪花叠在六角星上，已直接产出 `icon-source.png / icon.png /
32x32.png / 128x128.png / 128x128@2x.png / icon.ico`。需要重画时：

```powershell
pwsh -File scripts/generate-icon.ps1
```

如需面向 macOS 出 `.icns`、或想要 Tauri 官方的全套尺寸：

```bash
yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png
```

### Claude Code 数据位置

应用未来读取的 Claude Code 状态默认在：

```
C:\Users\<you>\.claude\
  sessions\<pid>.json   # 单进程会话元数据
  history.jsonl         # 命令历史，包含 sessionId + cwd
  projects\<encoded>\   # 按 cwd 编码的项目目录
```

当前阶段只用了内置 stub 数据，尚未真的去读这些文件。

## 与 Momo 的关系

Lilia 复用了 Momo 的几样东西，目的是保持视觉一致：

- 同一套 CSS 变量与浅深双色（`styles.css`）。
- Activity bar + Secondary panel + 自绘 TitleBar 的 VS Code 风格 shell。
- Tauri 2 窗口背景跟随系统主题。

不复用的部分：托盘 / 小组件窗口 / SQLite / WebDAV 同步等，
Lilia 当前不需要，留待后续按需引入。
