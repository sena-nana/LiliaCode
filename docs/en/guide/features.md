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
- [ ] Intelligent model selection: automatically choose model level and reasoning intensity based on request type.
- [x] File context: mention files, directories, images, and other context with `@`.
- [ ] Slash commands: support backend-native commands and project-defined commands.

## Claude Code Integration

- [x] Claude conversations: start new conversations and continue history sessions in LiliaCode.
- [x] Claude Skills: manage user-level and project-level Claude Skills.
- [x] Claude MCP management: add, edit, and remove external Claude MCP servers in the UI.
- [ ] Claude Plugins: fully manage Claude Plugin installation, enablement, updates, and scope.
- [ ] Claude Hooks: manage Claude Code Hooks and show execution results.
- [ ] Claude Subagents: display and schedule Claude Code Subagents or custom agents.

## Codex Integration

- [x] Codex conversations: start new conversations and continue history sessions in LiliaCode.
- [x] Codex process display: show Codex reasoning, commands, file changes, searches, and final replies.
- [x] Codex environment checks: show whether the Codex CLI, API, and connection state are available.
- [x] Codex MCP discovery: reuse Codex configuration to connect MCP servers.
- [ ] Codex MCP management: add, edit, and remove Codex MCP servers inside LiliaCode.
- [ ] Codex profiles: support profiles, sandbox and approval presets, and project-level configuration.
- [ ] Codex workflows: support common flows such as code review, fix suggestions, and batch changes.
- [ ] Built-in browser interaction: interact with users or debug code through IAB.

## LiliaCode-Specific Features

- [ ] Project management: manage local projects and GitHub-cloned projects, and view project-level progress, data, and cost.
- [ ] Task-based conversations: manage conversations as tasks for project-level scheduling.
- [ ] Task tree: manage parent-child tasks, dependencies, and blockers.
- [ ] Automatic orchestration: schedule multiple agents based on task state, dependencies, and user strategy.
- [ ] Plugin system: expose capabilities that change agent behavior as selectable plugins.
- [ ] Memory: save user-level and project-level memory, and help agents use it at the right time.
- [ ] Roadmap and milestones: show engineering progress across weeks and versions.
- [ ] Helper agents: run lower-cost agents in a session to supervise and assist the main agent.
- [ ] MutsukiCore integration: support remote task execution and mobile access.
