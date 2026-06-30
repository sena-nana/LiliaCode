---
name: lilia-app-coding
description: Coding workflow for final Lilia desktop applications. Use when Codex implements features, fixes bugs, refactors application code, adds routes or commands, changes cross-end data contracts, touches Vue pages under src/features, updates apps/desktop/src/router.ts or apps/desktop/src command/service wiring, or modifies app-owned Tauri Rust code.
---

# Lilia App Coding

## Start With Context

- Read the relevant module, data contract, route, command map, tests, and LiliaUI ownership boundary before editing.
- Use CodeGraph first when the repository is indexed and the task requires understanding code flow.
- Run `yarn verify:agent-debug` early when the app boundary or recommended verification commands are unclear. Use `yarn verify:agent-debug` for machine-readable output.
- For complex tasks, split work into clear sub-tasks and use subagents only where the boundary is clean enough for independent investigation or validation.

## Ownership

- Final apps own app configuration, routes, commands, business pages, and app-specific Tauri Rust boundaries.
- Put new business pages and feature logic under `src/features/<feature>/`.
- Connect business pages through `apps/desktop/src/router.ts` with async lazy imports unless there is a clear reason not to.
- Keep app command registration in `apps/desktop/src command/service wiring`.
- Keep app-specific Rust commands, state, and permissions inside `apps/desktop/src-tauri`.
- Do not copy public UI, shell, settings, menus, theme, global CSS, build wrappers, config sync, template checks, default assets, or window-state runtime code from LiliaUI into the app.

## Implementation Rules

- Fix root causes at the correct boundary. Do not patch symptoms with local workarounds.
- Preserve existing structures, names, and visible behavior unless the task requires changing them.
- Keep changes scoped to the requested feature or bug.
- Before changing a cross-end contract, define the boundary first, then update frontend, backend, permissions, and functional tests together.
- Do not display technical explanations in the UI.
- Do not add controls, routes, sidebar entries, commands, or disabled placeholders that are not connected to real behavior.
- Use `$lilia-agent-debug` when adding or changing `data-agent-id`, debug harnesses, `yarn verify:agent-debug`, or desktop replay support.
- When adding Agent, automation, timeline, permission, or approval behavior, define the user-visible workflow, runtime command, event shape, persistence, and fallback before wiring UI.
- Keep provider-specific or experimental payloads behind adapter/runtime boundaries. UI should use app or Lilia-level contracts and round-trip opaque provider context only when required.
- Prefer simple data flow over new abstractions. Add an abstraction only when it removes real duplication or matches an existing local pattern.
- Avoid comments that restate code. Put long-lived context, tradeoffs, or unresolved design notes in docs only when they are useful to future maintainers.
- Never overwrite user or other-agent changes. If nearby files are dirty, inspect and work with those changes.

## Frontend Pattern

- Start feature UI from `src/features/<feature>/`.
- Keep shared application wiring in `apps/desktop/src/app.config.ts`, `apps/desktop/src/router.ts`, and the relevant command or service modules.
- Use LiliaUI components, styles, CSS tokens, and shell conventions before adding local UI.
- Keep business component styles scoped.
- Ensure text, controls, loading state, empty state, and error state are stable on desktop and narrow viewports.

## Tauri Pattern

- Treat `apps/desktop/src-tauri` as the final app's Rust boundary only.
- Keep command names, payload shapes, permissions, and frontend invocations synchronized.
- Use typed request and response structures when a command has structured data.
- Update `apps/desktop/src-tauri/capabilities/default.json` when a new command or plugin permission needs frontend access.
- Leave `tauri-plugin-lilia` behavior in LiliaUI unless the change is truly app-specific.

## Before Finishing

- Remove duplicate branches, dead state, unused helper functions, and comments that only narrate the code.
- Confirm no fake UI or unconnected action was introduced.
- Confirm `$lilia-agent-debug` requirements are met when the changed flow needs Agent/debug validation.
- Run the smallest meaningful validation for the changed behavior, or explain why validation was not run.
