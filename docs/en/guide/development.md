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
│       │   ├── styles/         # Theme tokens, standard components, shell, and lazy page styles
│       │   ├── router.ts
│       │   └── mainBootstrap.ts
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

After pushing to `main`, the Pages workflow continues to publish the documentation site automatically. Before publishing a Windows desktop installer, sync and check the four version sources: root `package.json`, `apps/desktop/package.json`, `apps/desktop/src-tauri/Cargo.toml`, and `apps/desktop/src-tauri/tauri.conf.json`. They must match the release tag without the leading `v`.

```bash
yarn release:check --tag v1.0.0-beta.1
```

After the check passes, push a `v*` tag:

```bash
git tag v1.0.0-beta.1 && git push origin v1.0.0-beta.1
```

The release workflow runs `yarn verify` and `yarn release:check --tag <tag>` first, then builds the Windows Tauri NSIS bundle and uploads it to a draft GitHub Release. Installer asset names are checked against `LiliaCode_<version>_x64-setup.*`; the draft release includes a first-release checklist and generated release notes.

After the draft Release is created, CI runs the repeatable Windows installer smoke with `yarn release:smoke:windows --tag <tag>`. The same script can be run locally with `yarn release:smoke:windows --installer path/to/LiliaCode_<version>_x64-setup.exe`. It verifies install, main-window launch, opening a project from a fresh PowerShell or cmd with `liliacode <test-project-path>`, and removal of `liliacode` from fresh PATH after uninstall. Before publishing the release, record the installer smoke result in the Windows verification section of the Release body. Release artifacts are signed with `tauri-signing.key` for Tauri signing. They also do not include macOS notarization, Linux/macOS installers, or Tauri updater auto-update support. During the first-release phase, users upgrade manually by downloading and installing the newer Windows installer.

Use `docs/github/release-template.md` as the source template when preparing the GitHub Release body.

## Icons

The Tauri icon source is `apps/desktop/src-tauri/icons/icon.png`. To regenerate the desktop PNG or ICO set with the Tauri CLI, run:

```bash
yarn icons:generate
```

`icons:tauri` is kept as the same generation entrypoint:

```bash
yarn icons:tauri
```

