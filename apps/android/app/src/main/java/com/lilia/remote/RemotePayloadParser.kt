package com.lilia.remote

import org.json.JSONArray
import org.json.JSONObject

object RemotePayloadParser {
    fun parseBridgeStatus(response: JSONObject): RemoteBridgeStatus {
        RemoteEnvelopeAdapter.throwIfError(response, "Bridge status failed")
        val status = response.optJSONObject("status") ?: error("Bridge status missing status")
        return RemoteBridgeStatus(
            hostEnabled = status.optBoolean("hostEnabled"),
            state = status.optString("state", "disconnected"),
            pcName = status.optString("pcName", "Lilia PC"),
            capabilities = parseCapabilities(status.optJSONObject("capabilities")),
        )
    }

    private fun parseCapabilities(capabilities: JSONObject?): RemoteCapabilities {
        if (capabilities == null) return RemoteCapabilities()
        return RemoteCapabilities(
            supportsTaskInbox = capabilities.optBoolean("supportsTaskInbox", true),
            supportsChatSend = capabilities.optBoolean("supportsChatSend", true),
            supportsInteractionResponse = capabilities.optBoolean("supportsInteractionResponse", true),
            supportsInterrupt = capabilities.optBoolean("supportsInterrupt", true),
        )
    }

    fun parseProviderStatus(payload: JSONObject): RemoteProviderStatus =
        RemoteProviderStatus(
            backend = payload.optString("backend").ifBlank { "unknown" },
            ready = payload.optBoolean("ready"),
        )

    fun parseTaskList(payload: JSONObject): List<RemoteTaskSummary> {
        val tasks = payload.optJSONArray("tasks") ?: JSONArray()
        return buildList {
            for (index in 0 until tasks.length()) {
                val task = tasks.getJSONObject(index)
                val taskId = task.optString("taskId").takeIf { it.isNotBlank() } ?: continue
                add(
                    RemoteTaskSummary(
                        taskId = taskId,
                        title = task.nonBlankString("title", "Untitled task"),
                        projectName = task.optNullableString("projectName"),
                        status = task.nonBlankString("status", "waiting"),
                        lastActivity = formatRemoteTime(task.optLong("createdAt", 0L)),
                        pendingAction = null,
                    ),
                )
            }
        }
    }

    fun parseTaskDetail(
        taskId: String,
        taskPayload: JSONObject,
        timelinePayload: JSONObject,
        pendingPayload: JSONObject,
    ): RemoteTaskDetail {
        val task = taskPayload.optJSONObject("task")
        val runtime = taskPayload.optJSONObject("runtime")
        val summary = RemoteTaskSummary(
            taskId = task?.firstNonBlankString("taskId", "id") ?: taskId,
            title = task?.nonBlankString("title", "Untitled task") ?: "Untitled task",
            projectName = task?.firstNonBlankString("projectName", "projectId"),
            status = task?.nonBlankString("status", "waiting") ?: "waiting",
            lastActivity = formatRemoteTime(task?.optLong("createdAt", 0L) ?: 0L),
            pendingAction = null,
        )
        return RemoteTaskDetail(
            task = summary,
            runtimePhase = runtime?.nonBlankString("phase", summary.status) ?: summary.status,
            timeline = parseTimeline(timelinePayload.optJSONArray("events") ?: JSONArray()),
            pendingInteraction = parsePending(summary.taskId, pendingPayload.optJSONArray("interactions") ?: JSONArray()),
        )
    }

    private fun parseTimeline(events: JSONArray): List<RemoteTimelineItem> = buildList {
        for (index in 0 until events.length()) {
            val event = events.getJSONObject(index)
            add(
                RemoteTimelineItem(
                    id = event.optString("id", "event-$index"),
                    title = timelineTitle(event),
                    summary = timelineSummary(event),
                    status = event.optString("status").ifBlank { "info" },
                ),
            )
        }
    }

    private fun timelineTitle(event: JSONObject): String =
        event.optString("title")
            .ifBlank { timelineKindLabel(event.optString("kind")) }

    private fun timelineSummary(event: JSONObject): String {
        val payload = event.optJSONObject("payload")
        return event.optString("summary")
            .takeUnless { event.isNull("summary") }
            ?.takeIf { it.isNotBlank() }
            ?: payload?.firstNonBlankString("content", "text", "summary", "message", "command", "path", "query", "url")
            ?: ""
    }

    private fun timelineKindLabel(kind: String): String =
        when (kind) {
            "message" -> "Message"
            "assistant_message" -> "Assistant"
            "user_message" -> "User"
            "reasoning" -> "Reasoning"
            "plan" -> "Plan"
            "goal" -> "Goal"
            "todo_list" -> "Todo list"
            "tool" -> "Tool"
            "tool_call" -> "Tool call"
            "tool_result" -> "Tool result"
            "tool_consent" -> "Tool request"
            "ask_user" -> "Question"
            "architecture_change" -> "Architecture change"
            "command" -> "Command"
            "subagent" -> "Subagent"
            "file_change" -> "File change"
            "file_read" -> "File read"
            "search" -> "Search"
            "web_fetch" -> "Web fetch"
            "mcp" -> "MCP"
            "web_search" -> "Web search"
            "diagnostic" -> "Diagnostic"
            "title_update" -> "Title update"
            "error" -> "Error"
            "turn" -> "Turn"
            else -> kind.ifBlank { "Event" }
        }

    private fun parsePending(taskId: String, interactions: JSONArray): PendingInteraction? {
        val row = firstReplyableInteraction(interactions) ?: return null
        val payload = row.optJSONObject("payload") ?: JSONObject()
        val kind = row.optString("kind")
        return PendingInteraction(
            taskId = row.optString("taskId").ifBlank { taskId },
            requestId = row.optString("requestId"),
            kind = kind,
            backend = row.optString("backend"),
            title = interactionTitle(kind, payload),
            body = interactionBody(kind, payload),
            approveLabel = approveLabel(kind, payload),
            declineLabel = declineLabel(kind, payload),
            raw = row,
        )
    }

    private fun firstReplyableInteraction(interactions: JSONArray): JSONObject? {
        for (index in 0 until interactions.length()) {
            val row = interactions.optJSONObject(index) ?: continue
            if (row.optString("requestId").isBlank()) continue
            if (row.optString("kind").isBlank()) continue
            return row
        }
        return null
    }

    private fun interactionTitle(kind: String, payload: JSONObject): String =
        payload.optString("title")
            .ifBlank { payload.optString("displayName") }
            .ifBlank { payload.optString("serverName") }
            .ifBlank { payload.optString("toolName") }
            .ifBlank {
                when (kind) {
                    "permission_approval" -> "Permission request"
                    "architecture_change" -> "Architecture change"
                    "mcp_elicitation" -> "MCP request"
                    "plan_approval" -> "Plan approval"
                    "ask_user" -> "Question"
                    "tool_consent" -> "Tool request"
                    else -> "Pending action"
                }
            }

    private fun interactionBody(kind: String, payload: JSONObject): String {
        val firstQuestion = payload.optJSONArray("questions")?.optJSONObject(0)
        val question = firstQuestion?.optString("question")?.takeIf { it.isNotBlank() }
        if (question != null) return question
        return payload.optString("reason")
            .ifBlank { payload.optString("description") }
            .ifBlank { payload.optString("message") }
            .ifBlank { architectureChangeSummary(payload) }
            .ifBlank {
                when (kind) {
                    "tool_consent" -> toolConsentSummary(payload)
                    "mcp_elicitation" -> "The PC runner is requesting input from ${payload.optString("serverName", "an MCP server")}."
                    "permission_approval" -> "The PC runner is requesting additional access."
                    else -> "Waiting for a decision on the PC runner."
                }
            }
    }

    private fun approveLabel(kind: String, payload: JSONObject): String {
        val firstQuestion = payload.optJSONArray("questions")?.optJSONObject(0)
        return firstQuestion?.optString("confirmLabel")?.takeIf { it.isNotBlank() }
            ?: when (kind) {
                "permission_approval", "tool_consent" -> "Allow"
                "architecture_change" -> "Apply"
                "mcp_elicitation" -> "Accept"
                "plan_approval" -> "Approve plan"
                "ask_user" -> "Answer"
                else -> "Approve"
            }
    }

    private fun declineLabel(kind: String, payload: JSONObject): String {
        val firstQuestion = payload.optJSONArray("questions")?.optJSONObject(0)
        return firstQuestion?.optString("cancelLabel")?.takeIf { it.isNotBlank() }
            ?: when (kind) {
                "permission_approval", "tool_consent" -> "Deny"
                "mcp_elicitation" -> "Decline"
                "plan_approval" -> "Decline plan"
                else -> "Decline"
            }
    }

    private fun toolConsentSummary(payload: JSONObject): String {
        val command = payload.optJSONObject("input")?.optString("cmd")?.takeIf { it.isNotBlank() }
            ?: payload.optString("blockedPath").takeIf { it.isNotBlank() }
        return command?.let { "Tool input: $it" } ?: "The PC runner wants to use a tool."
    }

    private fun architectureChangeSummary(payload: JSONObject): String {
        val changes = payload.optJSONArray("changes") ?: return ""
        if (changes.length() == 0) return ""
        return "${changes.length()} architecture change(s) requested"
    }

    private fun JSONObject.optNullableString(name: String): String? {
        if (!has(name) || isNull(name)) return null
        return optString(name).takeIf { it.isNotBlank() }
    }

    private fun JSONObject.nonBlankString(name: String, fallback: String): String =
        optString(name).takeIf { it.isNotBlank() } ?: fallback

    private fun JSONObject.firstNonBlankString(vararg names: String): String? {
        for (name in names) {
            if (!has(name) || isNull(name)) continue
            val value = optString(name).takeIf { it.isNotBlank() }
            if (value != null) return value
        }
        return null
    }

    private fun formatRemoteTime(value: Long): String {
        if (value <= 0L) return "No activity timestamp"
        return java.text.DateFormat.getDateTimeInstance(
            java.text.DateFormat.SHORT,
            java.text.DateFormat.SHORT,
        ).format(java.util.Date(value))
    }
}
