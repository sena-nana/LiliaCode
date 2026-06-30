---
name: lilia-agent-debug
description: Agent debugging workflow for Lilia desktop. Use when adding, changing, validating, or reviewing Agent debug support, data-agent-id targets, window.__liliaAgentDebug, yarn verify:agent-debug, tauri-driver readiness, debug-only UI instrumentation, or desktop replay/debug harness behavior.
---

# Lilia Agent Debug

## Core Rule

Treat Agent debugging as a real developer interface, not as visible product UI. Users should see normal application state; Agents should get stable hidden structure and dev-only debug APIs.

## Ownership

- Put shared frontend harness code in `@lilia/ui`.
- Put reusable readiness reports, target checks, and migrations in LiliaUI tooling when they are not Lilia-specific.
- Keep Lilia app code limited to app-owned `data-agent-id` targets, feature-specific scenarios, and thin script entries.
- Keep provider protocols, chat timelines, runner commands, and private scenario scripts in Lilia unless a reusable interface is deliberately extracted.

## Implementation Pattern

1. Start with `$lilia-app-boundary` when the change crosses LiliaUI, Tauri, contracts, or app-owned feature code.
2. Keep files small and single-purpose. Split harness work into env, types, logging, snapshots, actions, and installer modules when the logic grows.
3. Gate frontend debug APIs with explicit dev/test flags such as `VITE_LILIA_AGENT_DEBUG=1` or an agent-debug mode. Debug APIs must not install in normal production UI.
4. Expose stable `data-agent-id` values for primary controls, important rows, retry/recover actions, filters, tabs, dialogs, and destructive confirmations.
5. Name `data-agent-id` by functional path, not translated text, CSS class, DOM position, or layout: `home.start-card`, `settings.provider.save`, `tasks.row.<taskId>.open`.
6. Keep `data-agent-id` invisible and non-semantic to users. Do not add public technical instructions, automation labels, or debug-only copy.
7. When adding a script, prefer a thin entry and keep reusable reporting or replay helpers in LiliaUI tooling.

## Expected Interfaces

- `yarn verify:agent-debug`: returns readiness, important files, stable targets, relevant environment flags, and external tool availability.
- `window.__liliaAgentDebug.observe()`: returns route, viewport, active element, visible `data-agent-id` tree, and recent errors.
- `window.__liliaAgentDebug.act(...)`: operates by `data-agent-id`, not by text, class, coordinate, or screenshot matching.
- `window.__liliaAgentDebug.mark(...)`: records a debug marker without changing business data.
- `window.__liliaAgentDebug.getRecentErrors()`: exposes recent frontend errors for debugging.

## Desktop Replay

Use `tauri-driver` for desktop automation, but do not model it as an npm dependency.

- Detect `tauri-driver`, EdgeDriver or another WebDriver bridge in readiness reports.
- Treat missing desktop replay tools as a setup blocker only for replay scenarios, not for basic debug readiness.
- Write screenshots, logs, replay steps, and summary artifacts when implementing full replay in `@lilia/build`.
- Keep replay scenarios functional: assert route behavior, command effects, stable targets, invoke boundaries, or persisted records. Do not hard-match incidental text or logs.

## Validation

- For harness changes in LiliaUI, run focused UI tests plus the relevant package typecheck.
- For `@lilia/tools` report changes in LiliaUI, run focused tools tests and the relevant workspace typecheck.
- For Lilia script or target changes, run `yarn verify:agent-debug` and affected desktop tests; use `yarn verify:desktop:build` when frontend integration changed.
- If a full desktop replay is expected but cannot run, report the missing tool, command, artifact path if any, and remaining risk.
