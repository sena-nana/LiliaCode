package com.lilia.remote

const val UNTRUSTED_PC_MESSAGE = "This PC no longer trusts this phone. Pair again."

data class ActivePcSwitcherItem(
    val pc: SavedPc,
    val active: Boolean,
)

fun savedPcConnectionMatches(current: SavedPc?, candidate: SavedPc): Boolean =
    current != null && activePcConnectionMatches(current, candidate)

fun shouldIgnorePcResult(current: SavedPc?, candidate: SavedPc): Boolean =
    current == null || !savedPcConnectionMatches(current, candidate)

fun shouldClearActivePcForFailure(err: Throwable): Boolean =
    err is RemoteBridgeException && err.code == "unauthorized"

fun activePcSwitcherItems(
    activePc: SavedPc?,
    savedPcs: List<SavedPc>,
): List<ActivePcSwitcherItem> {
    val pcs = buildList {
        activePc?.let { add(it) }
        savedPcs.forEach { pc ->
            if (none { activePcConnectionMatches(it, pc) }) {
                add(pc)
            }
        }
    }
    return pcs.map { pc ->
        ActivePcSwitcherItem(
            pc = pc,
            active = activePc != null && activePcConnectionMatches(activePc, pc),
        )
    }
}

fun RemoteTaskDetail.withRelatedTasks(tasks: List<RemoteTaskSummary>): RemoteTaskDetail {
    val byId = LinkedHashMap<String, RemoteTaskSummary>()
    tasks.forEach { byId[it.taskId] = it }
    byId[task.taskId] = task
    return copy(relatedTasks = byId.values.toList())
}

fun RemoteTaskDetail.withOlderTimelinePage(page: RemoteTimelinePage): RemoteTaskDetail {
    val mergedTimeline = RemotePayloadParser.prependTimeline(timeline, page.events)
    val mergedPage = timelinePage.copy(
        events = mergedTimeline,
        beforeCursor = page.beforeCursor,
        hasMoreBefore = page.hasMoreBefore,
    )
    return copy(timeline = mergedTimeline, timelinePage = mergedPage)
}

fun syncInboxTaskStatus(
    tasks: List<RemoteTaskSummary>,
    detail: RemoteTaskDetail,
): List<RemoteTaskSummary> =
    tasks.map { task ->
        if (task.taskId == detail.task.taskId) {
            task.copy(
                status = detail.task.status,
                pendingAction = detail.pendingInteraction?.title,
            )
        } else {
            task
        }
    }

data class TaskRunBlockInfo(
    val reason: String,
    val chain: List<RemoteTaskSummary>,
)

fun taskRunBlockInfo(
    task: RemoteTaskSummary,
    tasks: List<RemoteTaskSummary>,
): TaskRunBlockInfo? {
    val byId = LinkedHashMap<String, RemoteTaskSummary>()
    tasks.forEach { byId[it.taskId] = it }
    byId[task.taskId] = task
    val dependencyBlock = dependencyRunBlockInfo(task, byId, listOf(task), LinkedHashSet())
    if (task.status == "blocked") {
        return TaskRunBlockInfo(
            reason = "This task is marked blocked and cannot start a session.",
            chain = dependencyBlock?.chain ?: listOf(task),
        )
    }
    return dependencyBlock
}

private fun dependencyRunBlockInfo(
    task: RemoteTaskSummary,
    byId: Map<String, RemoteTaskSummary>,
    chain: List<RemoteTaskSummary>,
    visiting: MutableSet<String>,
): TaskRunBlockInfo? {
    if (!visiting.add(task.taskId)) {
        return TaskRunBlockInfo(
            reason = "Task dependency cycle detected; cannot start a session.",
            chain = chain,
        )
    }
    for (dependencyId in task.dependsOn) {
        val dependency = byId[dependencyId] ?: continue
        val nextChain = chain + dependency
        if (visiting.contains(dependency.taskId)) {
            return TaskRunBlockInfo(
                reason = "Task dependency cycle detected; cannot start a session.",
                chain = nextChain,
            )
        }
        if (dependency.status != "done") {
            return TaskRunBlockInfo(
                reason = "Cannot send until dependency is done: ${dependency.title} (${taskStatusLabel(dependency.status)}).",
                chain = nextChain,
            )
        }
        dependencyRunBlockInfo(dependency, byId, nextChain, visiting)?.let {
            return it
        }
    }
    visiting.remove(task.taskId)
    return null
}

fun taskStatusLabel(status: String): String =
    when (status) {
        "draft" -> "Draft"
        "waiting" -> "Waiting"
        "running" -> "Running"
        "blocked" -> "Blocked"
        "done" -> "Done"
        "cancelled" -> "Cancelled"
        else -> status.ifBlank { "Waiting" }.replaceFirstChar { it.uppercase() }
    }

data class InteractionOptionQuestion(
    val questionId: String,
    val mode: String,
    val minSelections: Int,
    val maxSelections: Int?,
    val options: List<InteractionOption>,
)

data class InteractionOption(
    val id: String,
    val label: String,
    val description: String?,
)

fun interactionOptionQuestion(interaction: PendingInteraction): InteractionOptionQuestion? {
    if (interaction.kind != "ask_user" && interaction.kind != "plan_approval") return null
    val question = interaction.raw
        .optJSONObject("payload")
        ?.optJSONArray("questions")
        ?.optJSONObject(0)
        ?: return null
    val options = question.optJSONArray("options") ?: return null
    if (options.length() == 0) return null
    val mode = question.optString("mode").ifBlank { "single" }
    if (mode != "single" && mode != "multi") return null
    val parsed = buildList {
        for (index in 0 until options.length()) {
            val option = options.optJSONObject(index) ?: continue
            val label = option.optString("label")
                .ifBlank { option.optString("id") }
                .ifBlank { "Option ${index + 1}" }
            val id = option.optString("id")
                .ifBlank { label }
            add(
                InteractionOption(
                    id = id,
                    label = label,
                    description = option.optString("description").takeIf { it.isNotBlank() },
                ),
            )
        }
        if (question.optBoolean("allowOther")) {
            add(InteractionOption(id = "other", label = "Other", description = null))
        }
    }
    if (parsed.isEmpty()) return null
    val questionId = question.optString("id").ifBlank { "question-0" }
    val minSelections = if (mode == "multi") {
        question.optInt("minSelections", 1).coerceAtLeast(0)
    } else {
        1
    }
    val maxSelections = if (mode == "multi" && question.has("maxSelections")) {
        question.optInt("maxSelections", parsed.size).coerceAtLeast(minSelections)
    } else {
        null
    }
    return InteractionOptionQuestion(
        questionId = questionId,
        mode = mode,
        minSelections = minSelections,
        maxSelections = maxSelections,
        options = parsed,
    )
}

fun selectedOptionsByQuestion(
    question: InteractionOptionQuestion?,
    selectedOptions: Set<String>,
): Map<String, List<String>> {
    if (question == null || selectedOptions.isEmpty()) return emptyMap()
    val ordered = question.options
        .map { it.id }
        .filter { selectedOptions.contains(it) }
    if (ordered.isEmpty()) return emptyMap()
    return mapOf(question.questionId to ordered)
}

fun interactionCanSubmit(
    question: InteractionOptionQuestion?,
    selectedOptions: Set<String>,
    responseText: String,
): Boolean {
    if (question == null) return true
    if (selectedOptions.size < question.minSelections) return false
    if (question.maxSelections != null && selectedOptions.size > question.maxSelections) return false
    if (selectedOptions.contains("other") && responseText.trim().isEmpty()) return false
    return true
}

fun taskInboxUnsupportedMessage(): String =
    "Task inbox is not supported by this PC bridge."
