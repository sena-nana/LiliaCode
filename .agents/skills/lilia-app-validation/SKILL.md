---
name: lilia-app-validation
description: Validation strategy for final Lilia desktop application changes. Use when Codex needs to choose or report checks after app code, routes, commands, UI, Tauri Rust, dependencies, build config, documentation, tests, or LiliaUI dependency updates change.
---

# Lilia App Validation

## Choose The Smallest Meaningful Check

Run checks that validate real behavior affected by the change. Prefer targeted functional checks over broad or brittle assertions.

- Always consider `yarn verify:agent-debug` when app boundaries, important files, or recommended checks may have changed.
- Use `$lilia-agent-debug` to choose checks for UI main paths, `data-agent-id`, debug harnesses, and desktop replay support.
- Use `yarn verify:desktop:test` for route behavior, command wiring, component behavior, config synchronization expectations, and business logic.
- Use `yarn verify:desktop:build` for frontend compile, bundling, route import, and type integration risk.
- Use `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml` for app-owned Rust changes.
- Use `yarn verify` for broader final confidence when the change crosses frontend, Tauri, config, or build boundaries.

## Test Quality

- Add tests only for behavior changes or meaningful regression risk.
- Do not add tests for documentation-only, comment-only, or formatting-only changes.
- Do not write low-value tests that only hard-match log text, incidental strings, implementation comments, or snapshot-like markup.
- Do not use raw string matching as the main assertion when it does not prove the feature works; assert the behavior through roles, events, state changes, command effects, data results, or observable outcomes.
- Test user-visible behavior, command results, route outcomes, config synchronization, permission availability, or data-contract handling.
- For Agent debug changes, follow `$lilia-agent-debug` test-quality and gating requirements.
- Keep tests focused on the changed capability and existing public behavior.

## LiliaUI Dependency Changes

When a final app consumes a changed LiliaUI package:

- Validate LiliaUI in its source repository first: use `yarn typecheck` and `yarn verify:desktop:test` for package or UI changes.
- For `tauri-plugin-lilia`, run `cargo test -p tauri-plugin-lilia` in LiliaUI.
- After refreshing the final app dependency or lockfile, run at least `yarn verify:agent-debug`, `yarn verify:desktop:test`, and any affected build or Tauri check.

## Reporting

- State exactly which checks ran and whether they passed.
- If a check was skipped, explain why it was not necessary for the change.
- If a check cannot run, include the blocking error and remaining risk.
- Do not treat unrelated full-suite failures as blockers without confirming they touch the edited surface.
