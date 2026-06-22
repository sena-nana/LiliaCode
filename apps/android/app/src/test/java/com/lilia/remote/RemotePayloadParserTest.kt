package com.lilia.remote

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RemotePayloadParserTest {
    @Test
    fun parsesTaskListStatusFromSidebarSummary() {
        val tasks = RemotePayloadParser.parseTaskList(
            JSONObject(
                """
                {
                  "type": "tasks.list",
                  "tasks": [
                    {
                      "taskId": "task-1",
                      "projectId": "project-1",
                      "projectName": "Lilia",
                      "title": "Android remote",
                      "status": "running",
                      "createdAt": 1710000000000,
                      "pinned": false,
                      "route": "/projects/project-1/tasks/task-1"
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals(1, tasks.size)
        assertEquals("task-1", tasks[0].taskId)
        assertEquals("Lilia", tasks[0].projectName)
        assertEquals("running", tasks[0].status)
    }

    @Test
    fun parsesProviderStatusPayload() {
        val status = RemotePayloadParser.parseProviderStatus(
            JSONObject(
                """
                {
                  "type": "provider.status",
                  "backend": "codex",
                  "ready": true
                }
                """.trimIndent(),
            ),
        )

        assertEquals("codex", status.backend)
        assertEquals(true, status.ready)
    }

    @Test
    fun mergeTimelineDeduplicatesByIdAndPreservesOrder() {
        val merged = RemotePayloadParser.mergeTimeline(
            existing = listOf(
                RemoteTimelineItem(
                    id = "event-1",
                    title = "First",
                    summary = "Original first",
                    status = "completed",
                ),
                RemoteTimelineItem(
                    id = "event-2",
                    title = "Second",
                    summary = "Original second",
                    status = "running",
                ),
            ),
            incoming = listOf(
                RemoteTimelineItem(
                    id = "event-2",
                    title = "Second",
                    summary = "Updated second",
                    status = "completed",
                ),
                RemoteTimelineItem(
                    id = "event-3",
                    title = "Third",
                    summary = "New third",
                    status = "running",
                ),
            ),
        )

        assertEquals(listOf("event-1", "event-2", "event-3"), merged.map { it.id })
        assertEquals("Updated second", merged[1].summary)
        assertEquals("completed", merged[1].status)
    }

    @Test
    fun taskListFallsBackForBlankTitleAndStatus() {
        val tasks = RemotePayloadParser.parseTaskList(
            JSONObject(
                """
                {
                  "tasks": [
                    {
                      "taskId": "task-1",
                      "title": "",
                      "status": "",
                      "createdAt": 0
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals("Untitled task", tasks[0].title)
        assertEquals("waiting", tasks[0].status)
    }

    @Test
    fun taskListSkipsRowsWithoutTaskId() {
        val tasks = RemotePayloadParser.parseTaskList(
            JSONObject(
                """
                {
                  "tasks": [
                    { "taskId": "", "title": "Broken" },
                    { "title": "Missing" },
                    { "taskId": "task-1", "title": "Valid" }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals(1, tasks.size)
        assertEquals("task-1", tasks[0].taskId)
        assertEquals("Valid", tasks[0].title)
    }

    @Test
    fun parsesTaskDetailTimelineAndPendingInteraction() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject(
                """
                {
                  "type": "tasks.get",
                  "task": {
                    "id": "task-1",
                    "projectId": null,
                    "title": "Inbox chat",
                    "status": "blocked",
                    "createdAt": 1710000000000
                  },
                  "runtime": { "phase": "waiting_user" }
                }
                """.trimIndent(),
            ),
            timelinePayload = JSONObject(
                """
                {
                  "type": "timeline.snapshot",
                  "events": [
                    {
                      "id": "event-1",
                      "kind": "message",
                      "title": "Assistant",
                      "summary": "Ready",
                      "status": "success"
                    }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject(
                """
                {
                  "type": "interaction.pending",
                  "interactions": [
                    {
                      "taskId": "task-1",
                      "requestId": "request-1",
                      "kind": "ask_user",
                      "backend": "codex",
                      "payload": {
                        "title": "Confirm",
                        "questions": [
                          { "id": "choice", "question": "Continue?" }
                        ]
                      }
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals("blocked", detail.task.status)
        assertEquals("waiting_user", detail.runtimePhase)
        assertEquals("Assistant", detail.timeline[0].title)
        assertEquals("Ready", detail.timeline[0].summary)
        assertNotNull(detail.pendingInteraction)
        assertEquals("task-1", detail.pendingInteraction?.taskId)
        assertEquals("Continue?", detail.pendingInteraction?.body)
        assertEquals("Answer", detail.pendingInteraction?.approveLabel)
        assertEquals("Decline", detail.pendingInteraction?.declineLabel)
    }

    @Test
    fun taskDetailReadsSidebarStyleTaskFields() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "fallback-task",
            taskPayload = JSONObject(
                """
                {
                  "task": {
                    "taskId": "task-1",
                    "projectName": "Lilia",
                    "title": "Android remote",
                    "status": "running",
                    "createdAt": 1710000000000
                  }
                }
                """.trimIndent(),
            ),
            timelinePayload = JSONObject("""{ "events": [] }"""),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertEquals("task-1", detail.task.taskId)
        assertEquals("Lilia", detail.task.projectName)
        assertEquals("Android remote", detail.task.title)
        assertEquals("running", detail.task.status)
    }

    @Test
    fun taskDetailMarksRetryableTimelineErrors() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject(
                """
                {
                  "events": [
                    {
                      "id": "user-1",
                      "kind": "message",
                      "turnId": "turn-1",
                      "status": "success",
                      "payload": {
                        "role": "user",
                        "content": "Retry source"
                      }
                    },
                    {
                      "id": "error-1",
                      "kind": "error",
                      "turnId": "turn-1",
                      "status": "error",
                      "payload": {
                        "message": "Failed"
                      }
                    },
                    {
                      "id": "error-2",
                      "kind": "error",
                      "status": "error",
                      "payload": {
                        "retryContext": {
                          "content": "Explicit retry"
                        }
                      }
                    },
                    {
                      "id": "error-3",
                      "kind": "error",
                      "status": "error",
                      "payload": {
                        "message": "Detached"
                      }
                    }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertFalse(detail.timeline[0].retryable)
        assertTrue(detail.timeline[1].retryable)
        assertTrue(detail.timeline[2].retryable)
        assertFalse(detail.timeline[3].retryable)
    }

    @Test
    fun taskDetailFallsBackForBlankTitleAndStatus() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject(
                """
                {
                  "task": {
                    "id": "task-1",
                    "title": "",
                    "status": "",
                    "createdAt": 0
                  },
                  "runtime": { "phase": "" }
                }
                """.trimIndent(),
            ),
            timelinePayload = JSONObject("""{ "events": [] }"""),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertEquals("Untitled task", detail.task.title)
        assertEquals("waiting", detail.task.status)
        assertEquals("waiting", detail.runtimePhase)
    }

    @Test
    fun parsesToolConsentPendingInteractionLabelsAndSummary() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject("""{ "events": [] }"""),
            pendingPayload = JSONObject(
                """
                {
                  "interactions": [
                    {
                      "taskId": "task-1",
                      "requestId": "request-1",
                      "kind": "tool_consent",
                      "backend": "codex",
                      "payload": {
                        "toolName": "shell",
                        "input": { "cmd": "yarn android:verify" }
                      }
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals("shell", detail.pendingInteraction?.title)
        assertEquals("Tool input: yarn android:verify", detail.pendingInteraction?.body)
        assertEquals("Allow", detail.pendingInteraction?.approveLabel)
        assertEquals("Deny", detail.pendingInteraction?.declineLabel)
    }

    @Test
    fun pendingInteractionUsesFirstReplyableRow() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject("""{ "events": [] }"""),
            pendingPayload = JSONObject(
                """
                {
                  "interactions": [
                    { "requestId": "", "kind": "ask_user", "payload": { "title": "Broken" } },
                    { "requestId": "request-2", "kind": "", "payload": { "title": "Also broken" } },
                    {
                      "taskId": "task-1",
                      "requestId": "request-3",
                      "kind": "mcp_elicitation",
                      "backend": "codex",
                      "payload": {
                        "serverName": "docs",
                        "message": "Open MCP form"
                      }
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals("request-3", detail.pendingInteraction?.requestId)
        assertEquals("task-1", detail.pendingInteraction?.taskId)
        assertEquals("mcp_elicitation", detail.pendingInteraction?.kind)
        assertEquals("docs", detail.pendingInteraction?.title)
        assertEquals("Open MCP form", detail.pendingInteraction?.body)
    }

    @Test
    fun pendingInteractionIgnoresMalformedRows() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject("""{ "events": [] }"""),
            pendingPayload = JSONObject(
                """
                {
                  "interactions": [
                    { "requestId": "", "kind": "ask_user" },
                    { "requestId": "request-2", "kind": "" }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertEquals(null, detail.pendingInteraction)
    }

    @Test
    fun timelineFallsBackToKindLabelAndPayloadText() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject(
                """
                {
                  "events": [
                    {
                      "id": "event-1",
                      "kind": "message",
                      "status": "",
                      "title": "",
                      "summary": null,
                      "payload": { "content": "Hello from the runner" }
                    }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertEquals("Message", detail.timeline[0].title)
        assertEquals("Hello from the runner", detail.timeline[0].summary)
        assertEquals("info", detail.timeline[0].status)
    }

    @Test
    fun timelineLabelsCommonDesktopEventKinds() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject(
                """
                {
                  "events": [
                    { "id": "event-1", "kind": "assistant_message", "payload": { "content": "Done" } },
                    { "id": "event-2", "kind": "user_message", "payload": { "content": "Continue" } },
                    { "id": "event-3", "kind": "tool_result", "payload": { "summary": "Tests passed" } }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertEquals("Assistant", detail.timeline[0].title)
        assertEquals("Done", detail.timeline[0].summary)
        assertEquals("User", detail.timeline[1].title)
        assertEquals("Continue", detail.timeline[1].summary)
        assertEquals("Tool result", detail.timeline[2].title)
        assertEquals("Tests passed", detail.timeline[2].summary)
    }

    @Test
    fun timelineMarksCompletedAssistantMessagesAsBranchable() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject(
                """
                {
                  "events": [
                    {
                      "id": "event-1",
                      "kind": "message",
                      "turnId": "turn-1",
                      "status": "success",
                      "payload": {
                        "role": "assistant",
                        "content": "Ready"
                      }
                    },
                    {
                      "id": "event-2",
                      "kind": "message",
                      "turnId": "turn-2",
                      "status": "completed",
                      "payload": {
                        "role": "assistant",
                        "content": "Done"
                      }
                    }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        assertEquals("turn-1", detail.timeline[0].branchSourceTurnId)
        assertEquals("turn-2", detail.timeline[1].branchSourceTurnId)
    }

    @Test
    fun timelineOnlyMarksFinalAssistantMessageTurnsAsBranchable() {
        val detail = RemotePayloadParser.parseTaskDetail(
            taskId = "task-1",
            taskPayload = JSONObject("""{ "task": { "id": "task-1" } }"""),
            timelinePayload = JSONObject(
                """
                {
                  "events": [
                    {
                      "id": "user-message",
                      "kind": "message",
                      "turnId": "turn-user",
                      "status": "success",
                      "payload": { "role": "user", "content": "Continue" }
                    },
                    {
                      "id": "running-assistant",
                      "kind": "message",
                      "turnId": "turn-running",
                      "status": "running",
                      "payload": { "role": "assistant", "content": "Working" }
                    },
                    {
                      "id": "missing-turn",
                      "kind": "message",
                      "status": "success",
                      "payload": { "role": "assistant", "content": "Done" }
                    },
                    {
                      "id": "reasoning",
                      "kind": "reasoning",
                      "turnId": "turn-reasoning",
                      "status": "success",
                      "payload": { "role": "assistant", "content": "Done" }
                    }
                  ]
                }
                """.trimIndent(),
            ),
            pendingPayload = JSONObject("""{ "interactions": [] }"""),
        )

        detail.timeline.forEach { item ->
            assertEquals(null, item.branchSourceTurnId)
        }
    }
}
