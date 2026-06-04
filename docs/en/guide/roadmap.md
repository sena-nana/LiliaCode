# Roadmap

LiliaCode's roadmap is organized around one goal: turning agent sessions in software engineering into an observable, recoverable, and schedulable local task system.

## Near-Term Focus

- Complete a unified cross-backend interaction protocol for plan confirmations, tool confirmations, and agent questions.
- Improve Codex configuration management, including MCP servers, profiles, sandbox settings, and approval presets.
- Advance project-level management so local projects and GitHub-cloned projects can show progress, data, and cost.
- Stabilize task-based conversations so each conversation can reliably become a schedulable Task.

## Mid-Term Capabilities

- Task tree: manage parent-child tasks, dependencies, and blockers.
- Automatic orchestration: schedule multiple agents based on task state, dependencies, and user strategy.
- Plugin system: expose capabilities that change agent behavior as selectable plugins.
- Memory: save user-level and project-level memory, and help agents use it at the right time.

## Long-Term Direction

- Roadmap and milestones: show engineering progress across weeks and versions.
- Helper agents: run lower-cost agents in a session to supervise and assist the main agent.
- MutsukiCore integration: support remote task execution and mobile access.

## Development Status

LiliaCode is still changing quickly. The local database schema may be migrated or cleared as features evolve, so do not use it as the only place where important data is stored.
