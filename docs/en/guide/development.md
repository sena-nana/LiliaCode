# Development

The repository, package names, protocol names, and local configuration paths still use the `lilia` name to avoid breaking existing protocols and persistence paths.

## Project Structure

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
│               ├── plugins.rs  # Claude skills / plugins / MCP and Codex MCP discovery
│               └── lib.rs      # chat / settings / project / plugin IPC
└── packages/
    └── contracts/              # Shared TS types and timeline display rules
```

## Local Development

```bash
# 1. Install dependencies first
yarn install

# 2. Start only the Vite frontend
yarn dev

# 3. Start the Tauri desktop app, which requires a local Rust toolchain and WebView2
yarn tauri:dev

# 4. Run type checks, unit tests, Rust check, and contracts check
yarn verify
```

## Documentation Site

```bash
# Start the VitePress documentation site
yarn docs:dev

# Build static output for GitHub Pages
yarn docs:build

# Preview the built output locally
yarn docs:preview
```

GitHub Pages deployment is handled by the repository Actions workflow. After pushing to `main`, the site is built and published to `https://sena-nana.github.io/Lilia/`.

## Icons

The Tauri icon source is `apps/desktop/src-tauri/icons/icon.svg`, which embeds PNG data inside an SVG container. To regenerate the full PNG or ICO set, run:

```bash
pwsh -File scripts/generate-icon.ps1
```

For macOS `.icns` or a full size set, run:

```bash
yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png
```
