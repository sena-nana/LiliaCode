<!-- To replace the main window screenshot, keep the file name .github/assets/main-window.png to avoid README changes. -->

> English | [简体中文](README.zh-CN.md) | [Documentation](https://sena-nana.github.io/LiliaCode/)

> **Development Status**
>
> LiliaCode is still changing quickly. Core features are not fully complete, and the local database schema may change as new features land. Data may be cleared or migrated at any time, so do not rely on it as the only copy of important production work.

<p align="center">
  <img src="./apps/desktop/src-tauri/icons/icon.png" width="128" alt="LiliaCode logo" />
</p>

<h1 align="center">LiliaCode</h1>

<p align="center">
  <a href="https://qm.qq.com/q/WViyGEq8oA">
    <img alt="LiliaCode QQ group" src="https://img.shields.io/badge/LiliaCode-289582454-blue">
  </a>
</p>

<p align="center"><strong>A desktop client for agent-assisted software engineering.</strong></p>

<p align="center">LiliaCode organizes Claude Code and Codex workflows into recoverable, traceable, and schedulable local task state, helping developers manage project sessions, context, todos, and execution history.</p>

<p align="center">
  <img src="./.github/assets/main-window.png" alt="LiliaCode main window" />
</p>

---

## Product Positioning

LiliaCode is the software engineering workbench in the Lilia family. It does not simply wrap Claude Code or Codex in a chat window; instead, it adds a desktop-level organization layer for projects, tasks, sessions, permissions, and process state outside the agent execution layer.

It is built for developers who move engineering projects forward over time. Each conversation can be treated as a manageable task, while agent execution details and pending interactions are saved as local state. This provides the foundation for future task trees, automatic orchestration, and multi-agent collaboration.

## The Lilia Family

Lilia is a family of toolchain applications for high-collaboration agent workflows. Its goal is to connect different agents, execution environments, and engineering workflows into one observable, schedulable, and recoverable local workbench.

LiliaCode focuses on software engineering. Other applications in the same family may expand into additional agent collaboration workflows while sharing the same ideas around project state, task-based sessions, plugin capabilities, and human-agent collaboration boundaries.

## What Makes It Different

- Task-based sessions: manage conversations as tasks instead of only saving chat history.
- Local engineering state: record projects, sessions, todos, process details, and key interactions for easier recovery and continuation.
- Observable process: use a timeline to show agent reasoning, tool calls, command execution, file changes, and final responses.
- Non-interruptive interaction: move permission requests, plan confirmations, and agent questions into a pending area so they do not take over the input flow.
- Collaboration-ready structure: provide a shared shape for task trees, dependencies, orchestration, and helper agents.

Because LiliaCode uses its own session storage model instead of upstream CLI or SDK history formats, compatibility with raw original conversation history is not a goal. It prioritizes its own recoverable task structure.

## Feature Status

The list below tracks the current real integration surface. Only capabilities that are usable as user-facing features are marked complete; partially integrated and not-yet-integrated items remain unchecked. Last checked: 2026-06-04.

### Shared Agent Capabilities

- [x] Permission modes: choose execution scope by risk level, including full access, ask-first, and read-only modes, and map them into Claude / Codex runtime parameters.
- [x] Todo display: mirror Claude `TodoWrite` and Codex `todo_list` events to show the agent's current task list and progress.
- [x] Process timeline: distinguish and display agent reasoning, commands, tool calls, file changes, plans, and final replies.
- [x] Key node navigation: highlight important timeline nodes in the scrollbar and support quick jumps.
- [x] Non-interruptive interaction mode: move permission requests, agent questions, and plan confirmations into a pending area instead of taking over the input box.
- [x] Guidance queue: create, queue, and serially dispatch user guidance todos, with queue state recovered during active runs.
- [x] Basic MCP integration: Claude stdio MCP servers can be managed by Lilia and injected into runtime; Codex stdio MCP servers can be read from and managed in `~/.codex/config.toml`.
- [x] Unified interaction protocol: unify plan confirmations, tool confirmations, and agent questions across backends.
- [x] File context: mention files, directories, images, and other context with `@`, with pasted or dropped attachments also supported.
- [ ] Intelligent model selection: Lilia does not yet automatically choose model level or reasoning intensity based on request type.
- [ ] Slash commands: backend-native commands and project-defined commands are not yet supported.

### Claude Code Integration

- [x] Claude conversations: start new turns through Claude Agent SDK `query()` and save the SDK `session_id` so the same task can resume.
- [x] Claude Plan: mirror `ExitPlanMode`, route approval, cancellation, and revision through unified AskUser, then restore execution-phase permission mode after approval.
- [x] Claude Skills: manage user-level and project-level Skills, and pass enabled skill names into the SDK.
- [x] Claude tool display: normalize common tools including Bash, Read / Write / Edit / MultiEdit, Glob / Grep, NotebookEdit, WebSearch / WebFetch, TodoWrite, Task / Agent, and ExitPlanMode.
- [ ] Claude MCP management (partial): the UI can create, edit, delete, and enable stdio MCP servers; HTTP / SSE, OAuth, elicitation, tool policy, and SDK instance MCP are not yet integrated.
- [ ] Claude Plugins (partial): Lilia can discover and enable user-level local plugins, then pass enabled plugin paths to the SDK; installation, updates, project-level scope, and marketplace scope are not yet integrated.
- [ ] Claude Hooks (partial): the runtime registers a small SDK hook set and can display some hook lifecycle events; hooks configuration management and execution result panels are not yet available.
- [ ] Claude Subagents (partial): Task / Agent calls, task progress, and notifications can be displayed; subagent definitions, list management, and proactive scheduling UI are not yet available.

### Codex Integration

- [x] Codex conversations: start or resume Codex app-server threads and save runtime state by task.
- [x] Codex process display: show Codex reasoning, commands, file changes, searches, plans, and final replies.
- [x] Codex environment checks: show whether the Codex CLI, app-server, API, and connection state are available.
- [x] Codex Plan: enable the app-server experimental API, read the plan preset from `collaborationMode/list`, and pass `collaborationMode` to `turn/start`; after plan approval, Lilia explicitly returns to default mode for execution.
- [x] Codex approval bridge: command and file-change approvals enter unified tool confirmation with `additionalPermissions` / `availableDecisions`, and Lilia can execute user-edited Codex commands before steering the result back to Codex.
- [x] Codex MCP management: the UI can view, create, edit, delete, and enable user-level stdio MCP servers in `~/.codex/config.toml`; HTTP / OAuth / unknown transports remain read-only.
- [x] Codex profiles: support global and project-level profiles, reasoning effort, runtime workspace roots, controlled permissions, and sticky `thread/settings/update`.
- [x] Codex history: search, preview, import, and continue existing Codex app-server threads from the left sidebar import entry.
- [ ] Codex workflows: built-in code review, fix suggestion, and batch-change flows are not yet available.
- [ ] Built-in browser interaction: IAB-based user interaction or browser debugging is not yet available.

### LiliaCode-Specific Features

- [ ] Project management (partial): local projects, Git clone, project ordering, pinning, removal, and session counts are available; project-level progress, data, and cost views are not yet integrated.
- [x] Task-based conversations: conversations are persisted as tasks, with draft promotion, project conversations, orphan conversations, archiving, pinning, and ordering.
- [ ] Task tree (partial): the data layer has `parent_id` and `depends_on`, but full parent-child tree, dependency view, and blocker management UI are not yet available.
- [ ] Plugin system (partial): Claude Skills / Plugins / MCP and Codex MCP management can feed runtime extensions; a generic plugin system with selectable behavior plugins is not yet complete.
- [ ] Memory (partial): the project Memory tab and extension-host context candidate path exist; user-level / project-level memory storage, retrieval, and automatic injection are not yet user-facing.
- [ ] Roadmap and milestones (partial): the project Roadmap tab and documentation template exist; cross-week and cross-version progress data views are not yet integrated.
- [ ] Automatic orchestration: Lilia does not yet schedule multiple agents based on task state, dependencies, or user strategy.
- [ ] Helper agents: lower-cost agents do not yet run inside a session to supervise or assist the main agent.
- [ ] MutsukiCore integration: remote task execution and mobile access are not yet available.

## Project Structure

> The repository, package names, protocol names, and local configuration paths still use the `lilia` name to avoid breaking existing protocols and persistence paths.

```text
Lilia/
├── apps/
│   └── desktop/                # Main app: Vue 3 + Tauri 2
│       ├── src/
│       │   ├── layouts/        # AppShell / SecondaryPanel / TitleBar
│       │   ├── components/     # ViewTabs / TodoFloat / ChatComposer, etc.
│       │   ├── pages/          # project/ProjectShell / TaskDetail / Settings
│       │   ├── services/       # projectsStore / tasksStore / todos / chat
│       │   ├── router.ts
│       │   └── styles.css
│       └── src-tauri/          # Tauri 2 Rust side
│           └── src/
│               ├── store.rs    # lilia-store: SQLite + r2d2 + migrations
│               ├── todos.rs    # Intercepts TodoWrite / todo_list events -> TaskTodo upsert
│               ├── plugins.rs  # Claude skills / plugins / MCP and Codex MCP management
│               └── lib.rs      # chat / settings / project / plugin IPC
└── packages/
    └── contracts/              # Shared TS types and timeline display rules
```

## Early Development

LiliaCode uses Yarn 4.14.1 through Corepack. Enable Corepack first, then run contributor commands from the repository root through the root `yarn ...` scripts. `npm`, `pnpm`, global Yarn 1.x, and direct workspace script entrypoints are guarded and not supported as the contributor path.

```bash
# 1) Enable Corepack and activate the repository Yarn version
corepack enable
corepack prepare yarn@4.14.1 --activate

# 2) Install dependencies
yarn install

# 3) Start only the Vite frontend
yarn dev

# 4) Start the Tauri desktop app (requires a local Rust toolchain and WebView2)
yarn tauri:dev

# 5) Run type checks, unit tests, Rust check, and contracts check
yarn verify

# 6) Start, build, or preview the documentation site
yarn docs:dev
yarn docs:build
yarn docs:preview
```

If `yarn --version` still reports `1.x` after enabling Corepack, run commands through Corepack explicitly, for example `corepack yarn install` and `corepack yarn dev`. Repository scripts and workspace scripts enforce the same package-manager check so contributors hit one Corepack-managed Yarn path.

The Tauri icon source is [apps/desktop/src-tauri/icons/icon.svg](apps/desktop/src-tauri/icons/icon.svg), which is an embedded PNG inside an SVG container. To regenerate the full PNG / ICO set, run `yarn icons:generate`. For macOS `.icns` or a full size set, run `yarn icons:tauri`.

## Thanks

- Codex provided important references for interface design and interaction organization; LiliaCode continues to iterate on top of those ideas.
