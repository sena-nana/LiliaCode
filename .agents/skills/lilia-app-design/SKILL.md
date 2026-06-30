---
name: lilia-app-design
description: Design and interaction standards for final Lilia desktop applications. Use when Codex designs, implements, reviews, or fixes application pages, sidebar entries, empty/loading/error states, cards, dialogs, menus, scoped styles, visual hierarchy, or any UI behavior in a Lilia app that consumes @lilia/ui.
---

# Lilia App Design

## Core Direction

Design final applications as restrained engineering tools. Prioritize clear position, current state, available actions, and decisions that need user attention.

Every main view must answer:

- Where am I?
- What is the current state?
- Does the user need to act?

Never show technical implementation notes, roadmap placeholders, or UI that looks functional but is not wired to real behavior. Sidebar items, buttons, menu entries, disabled controls, and status surfaces must represent actual reachable state or real unavailable state.

## Layout

- Use `@lilia/ui` for the desktop shell, titlebar, sidebar, settings page, menu language, theme, and page base classes.
- Put final app business pages in the main workspace only. Do not rebuild the shell structure inside an app page.
- Configure navigation, footer status, and settings entry through `apps/desktop/src/app.config.ts`.
- Connect pages through `apps/desktop/src/router.ts`, preferably with async component imports.
- Avoid landing-page composition, hero blocks, oversized headings, decorative panels, marketing card streams, and nested cards.
- Use cards only for independent information groups, repeated items, dialogs, and actual tool containers.

## Visual Language

- Keep the interface quiet, dense enough for repeated work, and easy to scan.
- Use this hierarchy: main content > current state > process information and secondary actions.
- Use short, direct, actionable copy. Buttons name actions, status text states facts, hints explain impact.
- Prefer icon buttons for familiar tools such as collapse, search, settings, expand, and window controls.
- Use the LiliaUI `@lucide/vue` icon convention when an icon exists.
- Use text buttons when the action needs explicit wording.
- Rows and controls must have clear hover, active, muted, disabled, loading, empty, and error states without layout shift.

## Tokens And Styles

Use CSS variables from `@lilia/ui/styles.css`. Do not create a second public color system inside the final app.

- Surface: `--bg`, `--bg-elev`, `--bg-subtle`
- Interaction: `--bg-hover`, `--bg-active`, `--border-soft`, `--border`, `--border-strong`
- Text: `--text`, `--text-muted`, `--text-faint`
- Accent: `--accent`, `--accent-strong`, `--accent-soft`, `--accent-text`
- State: `--ok`, `--warn`, `--err`, `--ok-soft`, `--warn-soft`, `--err-soft`

Use soft state tokens only for state backgrounds, selected backgrounds, dangerous hover, or confirmation states. Do not turn them into large page backgrounds or decorative blocks.

Keep app-specific CSS in the business component's scoped style. Move cross-page, shell-specific, or component-system styles to LiliaUI, then update the app dependency.

## Page Surfaces

- Use `.page-header`, `.card`, `.kv`, and other LiliaUI page classes before adding local CSS.
- Page titles should stay compact: about 18px/600 for page headings, 13px muted text for descriptions.
- Keep page headers for orientation and state, not hero content.
- Cards use 8px radius, modest padding, `--border`, and stable dimensions where content can change.
- Card headings should feel like group labels, not marketing headlines.
- Key-value layouts must handle long paths, versions, and identifiers with wrapping.
- Menus, dropdowns, context menus, and confirm dialogs should use LiliaUI components and pass real items and handlers.
- Dangerous actions use error color only for dangerous hover, pending, or confirmation state.

## Motion And State

- Keep hover, active, border, and text color transitions around 0.12s.
- Avoid attention-grabbing movement, scaling, strong shadows, or saturated decorative color.
- Respect `prefers-reduced-motion: reduce`.
- Loading, empty, and disabled states must not resize controls or shift surrounding layout.

## Agent-Friendly UI

Use `$lilia-agent-debug` for detailed Agent debug implementation and validation rules. For design work, keep the user-facing UI normal while exposing stable hidden structure.

- Keep `data-agent-id` invisible and non-semantic to users. Do not add public technical instructions, automation labels, or debug-only copy to the UI.
- Important state must be visible as product state: pending approval, blocked work, failed action, empty result, loading, unavailable provider, and recoverable error all need clear user-facing states and real actions where applicable.
- If a visible action cannot be executed, show a truthful unavailable state or remove the action. Do not present placeholder buttons, fake menus, fake sidebar items, or unconnected Agent affordances.

## Review Checklist

- The UI still feels like a restrained engineering tool.
- The strongest visual weight is on real user content and current state.
- No public UI, shell behavior, or styling system was copied into the app.
- New colors come from LiliaUI tokens or belong in LiliaUI first.
- App-specific styles are scoped to business components.
- Navigation and actions are real, reachable, and wired.
- `$lilia-agent-debug` is followed for Agent-operated flows.
- No debug-only Agent affordance or technical implementation explanation leaks into production UI.
- Light and dark themes remain readable.
- Hover, active, loading, empty, and disabled states keep stable geometry.
- Settings, shell, menus, theme, default assets, and window state remain owned by LiliaUI.
