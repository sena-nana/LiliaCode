package com.lilia.remote

import org.json.JSONArray
import org.json.JSONObject

object RemotePayloadParser {
    private val branchableTimelineStatuses = setOf("success", "completed", "done")
    private val timelineDetailWhitespace = Regex("\\s+")
    private const val TIMELINE_DETAIL_VALUE_MAX_LENGTH = 600

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
            supportsTimelineSubscription = capabilities.optBoolean("supportsTimelineSubscription", true),
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

    fun parseTaskState(
        taskId: String,
        taskPayload: JSONObject,
        pendingPayload: JSONObject,
    ): RemoteTaskState {
        val task = taskPayload.optJSONObject("task")
        val runtime = taskPayload.optJSONObject("runtime")
        val summary = parseTaskSummary(taskId, task)
        return RemoteTaskState(
            task = summary,
            runtimePhase = runtime?.nonBlankString("phase", summary.status) ?: summary.status,
            pendingInteraction = parsePending(summary.taskId, pendingPayload.optJSONArray("interactions") ?: JSONArray()),
        )
    }

    fun parseTaskDetail(
        taskId: String,
        taskPayload: JSONObject,
        timelinePayload: JSONObject,
        pendingPayload: JSONObject,
    ): RemoteTaskDetail {
        val state = parseTaskState(taskId, taskPayload, pendingPayload)
        return RemoteTaskDetail(
            task = state.task,
            runtimePhase = state.runtimePhase,
            timeline = parseTimelinePayload(timelinePayload),
            pendingInteraction = state.pendingInteraction,
        )
    }

    fun parseTimelinePayload(payload: JSONObject): List<RemoteTimelineItem> =
        parseTimeline(payload.optJSONArray("events") ?: JSONArray())

    fun mergeTimeline(
        existing: List<RemoteTimelineItem>,
        incoming: List<RemoteTimelineItem>,
    ): List<RemoteTimelineItem> {
        if (existing.isEmpty()) return incoming
        if (incoming.isEmpty()) return existing
        val merged = LinkedHashMap<String, RemoteTimelineItem>()
        existing.forEach { item -> merged[item.id] = item }
        incoming.forEach { item -> merged[item.id] = item }
        return merged.values.toList()
    }

    private fun parseTaskSummary(taskId: String, task: JSONObject?): RemoteTaskSummary =
        RemoteTaskSummary(
            taskId = task?.firstNonBlankString("taskId", "id") ?: taskId,
            title = task?.nonBlankString("title", "Untitled task") ?: "Untitled task",
            projectName = task?.firstNonBlankString("projectName", "projectId"),
            status = task?.nonBlankString("status", "waiting") ?: "waiting",
            lastActivity = formatRemoteTime(task?.optLong("createdAt", 0L) ?: 0L),
            pendingAction = null,
        )

    private fun parseTimeline(events: JSONArray): List<RemoteTimelineItem> {
        val rows = buildList {
            for (index in 0 until events.length()) {
                add(events.getJSONObject(index))
            }
        }
        return rows.mapIndexed { index, event ->
            RemoteTimelineItem(
                id = event.optString("id", "event-$index"),
                kind = event.optString("kind").ifBlank { "event" },
                title = timelineTitle(event),
                summary = timelineSummary(event),
                status = event.optString("status").ifBlank { "info" },
                role = timelineRole(event),
                details = timelineDetails(event),
                branchSourceTurnId = timelineBranchSourceTurnId(event),
                retryable = timelineRetryableError(event, rows),
            )
        }
    }

    private fun timelineRetryableError(event: JSONObject, events: List<JSONObject>): Boolean {
        if (event.optString("kind") != "error") return false
        if (retryContextHasPayload(event.optJSONObject("payload")?.optJSONObject("retryContext"))) {
            return true
        }
        val turnId = event.optString("turnId").takeIf { it.isNotBlank() } ?: return false
        return events.any { candidate ->
            candidate.optString("kind") == "message" &&
                candidate.optString("turnId") == turnId &&
                candidate.optJSONObject("payload")?.optString("role") == "user" &&
                retryContextHasPayload(candidate.optJSONObject("payload"))
        }
    }

    private fun retryContextHasPayload(payload: JSONObject?): Boolean {
        if (payload == null) return false
        if (payload.optString("content").isNotBlank()) return true
        val attachments = payload.optJSONArray("attachments")
        if (attachments != null && attachments.length() > 0) return true
        val conversationReferences = payload.optJSONArray("conversationReferences")
        return conversationReferences != null && conversationReferences.length() > 0
    }

    private fun timelineBranchSourceTurnId(event: JSONObject): String? {
        if (event.optString("kind") != "message") return null
        val payload = event.optJSONObject("payload") ?: return null
        if (payload.optString("role") != "assistant") return null
        if (event.optString("status") !in branchableTimelineStatuses) return null
        return event.optString("turnId").takeIf { it.isNotBlank() }
    }

    private fun timelineTitle(event: JSONObject): String {
        val title = event.optString("title").takeIf { it.isNotBlank() }
        if (title != null) return title
        if (event.optString("kind") == "message") {
            when (timelineRole(event)) {
                "assistant" -> return "Assistant"
                "user" -> return "User"
            }
        }
        return timelineKindLabel(event.optString("kind"))
    }

    private fun timelineRole(event: JSONObject): String? =
        event.optJSONObject("payload")
            ?.optString("role")
            ?.takeIf { it.isNotBlank() }

    private fun timelineSummary(event: JSONObject): String {
        val payload = event.optJSONObject("payload")
        return event.optString("summary")
            .takeUnless { event.isNull("summary") }
            ?.takeIf { it.isNotBlank() }
            ?: payload?.firstNonBlankString(
                "content",
                "text",
                "summary",
                "message",
                "error",
                "reason",
                "details",
                "command",
                "cmd",
                "path",
                "query",
                "url",
                "result",
                "response",
                "output",
                "stdout",
                "stderr",
            )
            ?: ""
    }

    private fun timelineDetails(event: JSONObject): List<RemoteTimelineDetail> {
        val payload = event.optJSONObject("payload") ?: JSONObject()
        return listOfNotNull(
            timelineDetail(
                "tool",
                payload.firstNonBlankString("toolName", "name", "tool", "function", "hookName"),
            ),
            timelineDetail("command", payload.firstNonBlankString("command", "cmd", "shellCommand")),
            timelineDetail("path", payload.firstNonBlankString("file", "filePath", "path")),
            timelineDetail("query", payload.firstNonBlankString("query", "url")),
            timelineDetail(
                "input",
                payload.timelineValueString("input", "arguments", "args", "parameters", "params", "request"),
            ),
            timelineDetail(
                if (isFailureStatus(event.optString("status"))) "error" else "output",
                payload.timelineValueString("result", "response", "output", "stdout", "stderr"),
            ),
            timelineDetail("code", payload.firstNonBlankString("code", "exitCode", "statusCode")),
            timelineDetail("turn", event.optString("turnId").takeIf { it.isNotBlank() }),
        )
    }

    private fun timelineDetail(
        label: String,
        value: String?,
    ): RemoteTimelineDetail? {
        val normalized = value?.replace(timelineDetailWhitespace, " ")?.trim().orEmpty()
        if (normalized.isBlank()) return null
        return RemoteTimelineDetail(
            label = label,
            value = normalized.truncate(TIMELINE_DETAIL_VALUE_MAX_LENGTH),
        )
    }

    private fun JSONObject.timelineValueString(vararg names: String): String? {
        for (name in names) {
            if (!has(name) || isNull(name)) continue
            val value = opt(name) ?: continue
            val text = when (value) {
                is JSONObject -> value.toString()
                is JSONArray -> value.toString()
                else -> value.toString()
            }.takeIf { it.isNotBlank() }
            if (text != null) return text
        }
        return null
    }

    private fun isFailureStatus(status: String): Boolean =
        status == "failed" || status == "error" || status == "cancelled"

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

    private fun String.truncate(maxLength: Int): String {
        if (length <= maxLength) return this
        return take(maxLength).trimEnd() + "..."
    }

    private fun formatRemoteTime(value: Long): String {
        if (value <= 0L) return "No activity timestamp"
        return java.text.DateFormat.getDateTimeInstance(
            java.text.DateFormat.SHORT,
            java.text.DateFormat.SHORT,
        ).format(java.util.Date(value))
    }
}
