# Feature Status

The list below describes the intended product capabilities. Checked items are currently usable as user-facing features; unchecked items are planned but not fully complete.

## Shared Agent Capabilities

- [x] Permission modes: choose execution scope by risk level, including full access, ask-first, and read-only modes.
- [x] Todo display: show the agent's current task list and progress.
- [x] Process timeline: distinguish and display agent reasoning, commands, tool calls, file changes, and replies.
- [x] Key node navigation: highlight important timeline nodes in the scrollbar and support quick jumps.
- [x] Non-interruptive interaction mode: move permission requests, agent questions, and plan confirmations into a pending area instead of taking over the input box.
- [x] Guidance queue: provide a priority action queue so user messages and plugin behavior enter a unified guidance flow.
- [x] Basic MCP integration: discover and connect MCP servers from agent configuration.
- [x] Unified interaction protocol: unify plan confirmations, tool confirmations, and agent questions across backends.
- [x] Unified Lilia protocol: the UI exposes Lilia operations only, with user-facing workflows and runtime commands dispatched by provider internally.
- [x] Built-in workflow types: route general task, frontend, refactor, test / verification, docs / prompt, Git / release, and architecture / memory flows as persistent `ChatWorkflow` payloads.
- [x] Intelligent model selection: automatically choose model level and reasoning intensity within the current backend based on workflow, plan mode, context size, and provider capability, with explicit send-time overrides.
- [x] File context: mention files, directories, images, and other context with `@`.
- [x] Slash commands: support the `/` command palette, built-in commands, and `.lilia/commands` project commands, with results written back to the task timeline. `v1.0` stabilizes the Lilia command path; full backend-native command support is part of `v2.0` conversation-level completion.

## Claude Code Integration

- [x] Claude conversations: start new conversations and continue history sessions in LiliaCode.
- [x] Claude prompt suggestions: show native Claude suggestions for the next user prompt.
- [x] Claude history import: search, preview, and import local Claude history sessions.
- [x] Claude Skills: manage user-level and project-level Claude Skills.
- [x] Claude MCP management: add, edit, and remove external Claude MCP servers in the UI.
- [x] Claude Lilia protocol: support built-in task workflows, review, fix suggestion, batch apply, runtime-command session fork, and local Goal / diagnostic handling.
- [x] Claude reasoning effort: automatically or manually set reasoning effort for a send and map it to Claude Agent SDK `effort` / adaptive thinking.
- [ ] Claude Plugins: fully manage Claude Plugin installation, enablement, updates, and scope. Target phase: `v2.0`.
- [x] Claude Hooks: support Hook injection, event reporting, and timeline records. `v1.0` stabilizes the current management surface; full governance and unified display belong to `v2.0`.
- [ ] Claude Subagents: display and schedule Claude Code Subagents or custom agents. Target phase: `v2.0`.

## Codex Integration

- [x] Codex conversations: start new conversations and continue history sessions in LiliaCode.
- [x] Codex process display: show Codex reasoning, commands, file changes, searches, and final replies.
- [x] Codex environment checks: show whether the Codex CLI, API, and connection state are available.
- [x] Codex MCP management: discover, create, edit, delete, and enable user-level stdio Codex MCP servers; HTTP / OAuth / unknown transports remain read-only.
- [x] Codex profiles: support profiles, reasoning effort, runtime workspace roots, controlled permissions, and project-level defaults.
- [x] Codex reasoning effort: automatically or manually set reasoning effort for a send; plan mode uses the selected effort for the turn.
- [x] Codex Lilia adapter: support built-in task workflows, review, fix suggestion, batch apply, compact, Goal, memory, config diagnostics, and background-terminal cleanup in the workflow layer, with session fork handled as a runtime command.
- [x] Built-in browser interaction: Codex can open and navigate IAB windows, collect page title / URL / screenshot metadata, and send results back to a running turn or message attachment. `v1.0` stabilizes the Windows first-release path; later expansion belongs to `v2.0`.

## LiliaCode-Specific Features

- [x] Project management: manage local projects and GitHub-cloned projects, and view project-level progress, data, and cost. Advanced metric interpretation and project-level assets belong to `v3.0`.
- [x] Task-based conversations: conversations are persisted as tasks, with draft promotion, project / orphan sessions, archiving, pinning, and ordering.
- [x] Task tree: manage parent-child tasks, dependencies, tree drag-and-drop, and blocker hints. `v1.0` stabilizes display and basic maintenance; automatic driving, blocker scheduling, task assignment, and failure rerouting belong to `v2.0`.
- [x] Built-in Lilia workflow types: Lilia's own workflow catalog routes through `lilia_task_workflow.kind` and is not managed as an external Skill on the Plugin / Skill page.
- [ ] Automatic orchestration: an automation execution framework exists; multi-Agent scheduling and strategy closure in the main conversation path target `v2.0`, while cron / webhook / subflow and advanced automation features target `v3.0`.
- [x] Plugin system: MCP / Skill / Plugin / Hook management and runtime injection are available. `v1.0` stabilizes the current management surface, `v2.0` improves governance and behavior policy, and `v3.0` covers distributable plugins / workflows.
- [x] Memory: manually save user-level and project-level memories and inject the Layer 1 baseline at session startup. `v2.0` adds browsing, filtering, and opportunity-window guidance; `v3.0` adds external-model retrieval and quality evaluation.
- [x] Roadmap and milestones: project roadmap, milestone, and task-milestone data links are implemented. `v1.0` stabilizes the current project roadmap entry; `v3.0` adds metric interpretation and advanced rollup views.
- [ ] Helper agents: run lower-cost agents in a session to supervise and assist the main agent. Target phase: `v2.0`.
- [x] Built-in Lilia protocol: keep a single built-in runtime path.
