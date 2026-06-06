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

This repository uses Yarn 4.14.1 through Corepack. Enable Corepack first, then run contributor commands from the repository root through the root `yarn ...` scripts. `npm`, `pnpm`, global Yarn 1.x, and direct workspace script entrypoints are guarded and not supported as the contributor path.

```bash
# 1. Enable Corepack and activate the repository Yarn version
corepack enable
corepack prepare yarn@4.14.1 --activate

# 2. Install dependencies
yarn install

# 3. Start only the Vite frontend
yarn dev

# 4. Start the Tauri desktop app, which requires a local Rust toolchain and WebView2
yarn tauri:dev

# 5. Run type checks, unit tests, Rust check, and contracts check
yarn verify
```

If `yarn --version` still reports `1.x` after enabling Corepack, run commands through Corepack explicitly, for example `corepack yarn install` and `corepack yarn dev`. Repository scripts and workspace scripts enforce the same package-manager check so contributors hit one Corepack-managed Yarn path.

## Documentation Site

```bash
# Start the VitePress documentation site
yarn docs:dev

# Build static output for GitHub Pages
yarn docs:build

# Preview the built output locally
yarn docs:preview
```

Run documentation commands from the repository root through the root `yarn ...` scripts.

GitHub Pages deployment is handled by the repository Actions workflow. After pushing to `main`, the site is built and published to `https://sena-nana.github.io/LiliaCode/`.

## CI/CD

GitHub Actions runs CI for pull requests to `main`, pushes to `main`, and manual workflow dispatches. CI runs `yarn verify` and builds the documentation site separately, covering desktop tests, frontend build, Tauri Rust check, contracts type check, and docs build.

After pushing to `main`, the Pages workflow continues to publish the documentation site automatically. To publish a Windows desktop installer, push a `v*` tag:

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

The release workflow runs `yarn verify` first, then builds the Windows Tauri bundle and uploads it to a draft GitHub Release. Current release artifacts do not include code signing, macOS notarization, Linux/macOS installers, or auto-update support.

## Icons

The Tauri icon source is `apps/desktop/src-tauri/icons/icon.svg`, which embeds PNG data inside an SVG container. To regenerate the full PNG or ICO set, run:

```bash
yarn icons:generate
```

For macOS `.icns` or a full size set, run:

```bash
yarn icons:tauri
```
