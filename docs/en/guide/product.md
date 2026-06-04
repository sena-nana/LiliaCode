# Product Positioning

LiliaCode is the software engineering workbench in the Lilia family. It does not simply wrap Claude Code or Codex in a chat window. Instead, it adds a desktop-level organization layer for projects, tasks, sessions, permissions, and process state outside the agent execution layer.

Each conversation can be treated as a manageable task. Agent execution details, pending interactions, and key context are saved as local state, providing the foundation for future task trees, automatic orchestration, and multi-agent collaboration.

## The Lilia Family

Lilia is a family of toolchain applications for high-collaboration agent workflows. Its goal is to connect different agents, execution environments, and engineering workflows into one observable, schedulable, and recoverable local workbench.

LiliaCode focuses on software engineering. Other applications in the same family may expand into additional agent collaboration workflows while sharing the same ideas around project state, task-based sessions, plugin capabilities, and human-agent collaboration boundaries.

## What Makes It Different

| Capability | Description |
| --- | --- |
| Task-based sessions | Manage conversations as tasks instead of only saving chat history. |
| Local engineering state | Record projects, sessions, todos, process details, and key interactions for easier recovery and continuation. |
| Observable process | Show agent reasoning, tool calls, command execution, file changes, and final responses in a timeline. |
| Non-interruptive interaction | Move permission requests, plan confirmations, and agent questions into a pending area so they do not take over the input flow. |
| Collaboration-ready structure | Provide a shared shape for task trees, dependencies, orchestration, and helper agents. |

## Storage Boundary

Because LiliaCode uses its own session storage model instead of upstream CLI or SDK history formats, compatibility with raw original conversation history is not a goal. It prioritizes its own recoverable task structure.
