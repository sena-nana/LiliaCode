---
name: lilia-app-boundary
description: Ownership rules for deciding whether a final Lilia desktop application change belongs in the app repository or in LiliaUI. Use when Codex touches shell behavior, titlebar, sidebar, settings, menus, theme, global CSS, config sync, build wrappers, default assets, window state, @lilia packages, tauri-plugin-lilia, app routes, commands, business pages, or app-owned Tauri code.
---

# Lilia App Boundary

## Decision Rule

Put behavior in the final app only when it is application-specific business logic, application configuration, page routing, command wiring, or an app-owned Tauri boundary.

Move or implement behavior in LiliaUI when it is reusable shell, UI system, styling, config, tooling, build, template check, default asset, or common Tauri runtime behavior.

## Final App Owns

- `app.config.json`: app name, product title, version, identifier, storage prefix, and app shell copy.
- `apps/desktop/src/app.config.ts`: app navigation, footer status, settings copy, and app-level shell configuration.
- `apps/desktop/src/router.ts`: final app routes and lazy-loaded business pages.
- `apps/desktop/src command/service wiring`: app command registration exposed to LiliaUI runtime.
- `src/features/**`: app business pages, workflows, state, and scoped styles.
- `apps/desktop/src-tauri/**`: app-specific Rust commands, app-specific state, capabilities, and Tauri configuration.
- `tests/**`: behavior tests for final app routes, commands, configuration, and business workflows.

## LiliaUI Owns

- `@lilia/ui`: shared components, desktop shell, titlebar, sidebar, settings page, menus, dialogs, theme, CSS tokens, reset, base controls, page classes, default copy patterns, and global UI behavior.
- `@lilia/config`: shared TypeScript, Vite, VitePress, and app config synchronization helpers.
- `@lilia/tools`: default assets, template checks, migrations, and surrounding tools.
- `@lilia/build`: dev, build, docs, Tauri run, and verify wrappers.
- `tauri-plugin-lilia`: main-window preparation, window state persistence, and shared Tauri runtime behavior.

Do not edit `node_modules/@lilia/*`. Modify the LiliaUI source repository, validate there, then update the final app dependency or lockfile.

## Common Decisions

- New business page or workflow: implement in Lilia under the relevant `apps/desktop/src` page or feature module, then wire through `apps/desktop/src/router.ts` and `apps/desktop/src/app.config.ts` when applicable.
- New app-specific command: implement in final app frontend and `apps/desktop/src-tauri`, then update capabilities and tests.
- Titlebar, sidebar, shell layout, settings, menu, theme, default resource, config sync, template check, build flow, or window-state change: implement in LiliaUI first.
- Repeated style or component pattern across final apps: implement in LiliaUI.
- One-off business visualization or workflow-specific style: keep scoped in the final app component.

## Agent-Friendly Ownership

Use `$lilia-agent-debug` for detailed Agent debugging workflows. Use this section only to decide ownership.

Put reusable Agent-friendly affordances in LiliaUI or its source packages:

- Stable `data-agent-id` on shared shell controls, shared dialogs, common menus, settings, titlebar, sidebar, and reusable LiliaUI components.
- Shared debug harness, template checks, dev-only instrumentation, screenshot/replay tooling, and agent-debug build wrappers.
- Common timeline, pending-action, permission, plan, markdown, or process-observation components only when they are generic UI primitives and do not embed Lilia-specific provider protocols.
- Shared display derivation helpers or contracts when multiple apps need the same event-to-UI mapping.

Keep final-app-specific Agent behavior in the final app:

- Business workflows, app routes, app-owned commands, app-specific Tauri state, and persistence.
- App-specific `data-agent-id` values for feature controls, rows, records, and actions.
- App-specific Agent timeline, approval, automation, or runner logic when the app owns the data contract or provider boundary.
- Feature validation scenarios that exercise real app behavior through the shared agent-debug harness.

If both sides are involved, define the public LiliaUI/component or debug interface first, then wire final-app behavior through that interface. Do not make the final app depend on private Lilia implementation details, provider payloads, or undocumented DOM structure.

## Guardrails

- Do not copy Lilia-specific paths, protocols, providers, task timelines, or verification scripts into final apps unless the app truly implements that capability.
- Do not duplicate shared shell or style code locally to make a quick fix.
- When unsure, inspect the current app code and LiliaUI package surface before choosing a boundary.
- If both sides must change, define the interface first, then update LiliaUI and the final app in that order.
