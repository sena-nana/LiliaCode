import type { ToolConsentRequest } from "@lilia/contracts";

export function toolConsentRequestFixture(
  overrides: Partial<ToolConsentRequest> = {},
): ToolConsentRequest {
  return {
    taskId: "task-1",
    turnId: "turn-1",
    backend: "claude",
    requestId: "tool-1",
    toolName: "Bash",
    input: { command: "pwd" },
    title: null,
    displayName: null,
    description: null,
    blockedPath: null,
    decisionReason: null,
    toolUseId: null,
    ...overrides,
  };
}
