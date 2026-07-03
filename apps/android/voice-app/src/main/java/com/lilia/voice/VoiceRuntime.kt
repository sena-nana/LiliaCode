package com.lilia.voice

import com.lilia.remote.RemoteTaskDetail
import com.lilia.remote.RemoteTaskSummary

enum class VoicePageId {
    HUB,
    PROJECT_SELECTION,
    SEARCH_DICTATION,
    SEARCH_RESULTS,
    PROJECT_DETAIL,
    LOG,
    START_COMMAND_DICTATION,
    CONFIRM,
}

enum class VoiceInputMode {
    COMMAND,
    DICTATION,
}

enum class VoiceRisk {
    SAFE,
    CONFIRM,
    DANGER,
}

data class VoiceFocus(
    val type: String,
    val label: String,
)

data class VoiceCommand(
    val intent: String,
    val phrases: List<String>,
    val risk: VoiceRisk = VoiceRisk.SAFE,
)

data class VoiceEntity(
    val id: String,
    val label: String,
    val aliases: List<String> = emptyList(),
)

data class VoicePageManifest(
    val pageId: VoicePageId,
    val title: String,
    val focus: VoiceFocus,
    val commands: List<VoiceCommand>,
    val entities: List<VoiceEntity> = emptyList(),
)

sealed interface VoiceAction {
    data object None : VoiceAction
    data object RefreshInbox : VoiceAction
    data class OpenTask(val taskId: String) : VoiceAction
    data class StartProcess(val taskId: String, val command: String) : VoiceAction
    data class StopProcess(val taskId: String) : VoiceAction
}

data class VoiceConfirmation(
    val message: String,
    val action: VoiceAction,
    val returnPage: VoicePageId,
)

data class VoiceRuntimeState(
    val pageId: VoicePageId = VoicePageId.HUB,
    val inputMode: VoiceInputMode = VoiceInputMode.COMMAND,
    val tasks: List<RemoteTaskSummary> = emptyList(),
    val selectedTask: RemoteTaskDetail? = null,
    val selectedIndex: Int = 0,
    val searchQuery: String = "",
    val searchResults: List<RemoteTaskSummary> = emptyList(),
    val pendingConfirmation: VoiceConfirmation? = null,
    val logPaused: Boolean = false,
    val logErrorsOnly: Boolean = false,
    val notice: String = "",
)

data class VoiceRuntimeResult(
    val state: VoiceRuntimeState,
    val action: VoiceAction = VoiceAction.None,
)

class VoiceRuntime {
    fun manifestFor(state: VoiceRuntimeState): VoicePageManifest =
        when (state.pageId) {
            VoicePageId.HUB -> VoicePageManifest(
                pageId = VoicePageId.HUB,
                title = "Home",
                focus = VoiceFocus("hub", "project entry"),
                commands = listOf(
                    VoiceCommand("open_project_selector", listOf("open", "open project", "打开", "打开项目")),
                    VoiceCommand("open_search", listOf("search", "搜索", "查找")),
                    VoiceCommand("show_running", listOf("running", "运行中", "查看运行中")),
                ),
            )
            VoicePageId.PROJECT_SELECTION -> VoicePageManifest(
                pageId = VoicePageId.PROJECT_SELECTION,
                title = "Select project",
                focus = VoiceFocus("list", "project list"),
                commands = selectionCommands(),
                entities = taskEntities(state.tasks),
            )
            VoicePageId.SEARCH_DICTATION -> VoicePageManifest(
                pageId = VoicePageId.SEARCH_DICTATION,
                title = "Search projects",
                focus = VoiceFocus("dictation", "search text"),
                commands = globalCommands(),
            )
            VoicePageId.SEARCH_RESULTS -> VoicePageManifest(
                pageId = VoicePageId.SEARCH_RESULTS,
                title = "Search results",
                focus = VoiceFocus("list", "matching projects"),
                commands = selectionCommands(),
                entities = taskEntities(state.searchResults),
            )
            VoicePageId.PROJECT_DETAIL -> VoicePageManifest(
                pageId = VoicePageId.PROJECT_DETAIL,
                title = state.selectedTask?.task?.title ?: "Project",
                focus = VoiceFocus("object", state.selectedTask?.task?.title ?: "current project"),
                commands = listOf(
                    VoiceCommand("start_project", listOf("start", "启动", "运行")),
                    VoiceCommand("stop_project", listOf("stop", "停止"), VoiceRisk.CONFIRM),
                    VoiceCommand("show_logs", listOf("logs", "查看日志", "日志")),
                    VoiceCommand("refresh", listOf("refresh", "刷新")),
                ) + globalCommands(),
            )
            VoicePageId.LOG -> VoicePageManifest(
                pageId = VoicePageId.LOG,
                title = "Logs",
                focus = VoiceFocus("stream", "current log stream"),
                commands = listOf(
                    VoiceCommand("pause_log", listOf("pause", "暂停")),
                    VoiceCommand("resume_log", listOf("continue", "继续")),
                    VoiceCommand("filter_errors", listOf("errors", "筛选错误", "错误")),
                    VoiceCommand("summarize_log", listOf("summary", "总结")),
                ) + globalCommands(),
            )
            VoicePageId.START_COMMAND_DICTATION -> VoicePageManifest(
                pageId = VoicePageId.START_COMMAND_DICTATION,
                title = "Start command",
                focus = VoiceFocus("dictation", "process command"),
                commands = globalCommands(),
            )
            VoicePageId.CONFIRM -> VoicePageManifest(
                pageId = VoicePageId.CONFIRM,
                title = "Confirm",
                focus = VoiceFocus("confirm", "pending action"),
                commands = listOf(
                    VoiceCommand("confirm", listOf("confirm", "yes", "确认", "执行", "是")),
                    VoiceCommand("cancel_confirm", listOf("cancel", "no", "取消", "不要", "否")),
                ),
            )
        }

    fun reduce(state: VoiceRuntimeState, transcript: String): VoiceRuntimeResult {
        val text = transcript.trim()
        if (text.isBlank()) return VoiceRuntimeResult(state)
        val normalized = normalize(text)

        state.pendingConfirmation?.let { confirmation ->
            if (isConfirm(normalized)) {
                return VoiceRuntimeResult(
                    state.copy(
                        pageId = confirmation.returnPage,
                        pendingConfirmation = null,
                        notice = confirmation.message,
                    ),
                    confirmation.action,
                )
            }
            if (isCancel(normalized)) {
                return VoiceRuntimeResult(
                    state.copy(
                        pageId = confirmation.returnPage,
                        pendingConfirmation = null,
                        notice = "Canceled.",
                    ),
                )
            }
        }

        handleGlobalCommand(state, normalized)?.let { return it }

        if (state.inputMode == VoiceInputMode.DICTATION) {
            return handleDictation(state, text)
        }

        return when (state.pageId) {
            VoicePageId.HUB -> handleHub(state, normalized)
            VoicePageId.PROJECT_SELECTION -> handleSelection(state, state.tasks, normalized)
            VoicePageId.SEARCH_RESULTS -> handleSelection(state, state.searchResults, normalized)
            VoicePageId.PROJECT_DETAIL -> handleProjectDetail(state, text, normalized)
            VoicePageId.LOG -> handleLog(state, normalized)
            VoicePageId.SEARCH_DICTATION,
            VoicePageId.START_COMMAND_DICTATION,
            VoicePageId.CONFIRM,
            -> VoiceRuntimeResult(state.copy(notice = "Listening for input."))
        }
    }

    fun isActionAllowed(state: VoiceRuntimeState, action: VoiceAction): Boolean =
        when (action) {
            VoiceAction.None -> true
            VoiceAction.RefreshInbox -> state.pageId == VoicePageId.HUB ||
                state.pageId == VoicePageId.PROJECT_SELECTION ||
                state.pageId == VoicePageId.PROJECT_DETAIL
            is VoiceAction.OpenTask -> state.pageId == VoicePageId.PROJECT_SELECTION ||
                state.pageId == VoicePageId.SEARCH_RESULTS
            is VoiceAction.StartProcess -> state.pageId == VoicePageId.PROJECT_DETAIL ||
                state.pageId == VoicePageId.START_COMMAND_DICTATION
            is VoiceAction.StopProcess -> state.pageId == VoicePageId.CONFIRM ||
                state.pageId == VoicePageId.PROJECT_DETAIL
        }

    fun withTasks(state: VoiceRuntimeState, tasks: List<RemoteTaskSummary>): VoiceRuntimeState =
        state.copy(tasks = tasks, searchResults = searchTasks(tasks, state.searchQuery))

    fun withTaskDetail(state: VoiceRuntimeState, detail: RemoteTaskDetail): VoiceRuntimeState =
        state.copy(
            pageId = VoicePageId.PROJECT_DETAIL,
            inputMode = VoiceInputMode.COMMAND,
            selectedTask = detail,
            notice = "",
        )

    private fun handleHub(state: VoiceRuntimeState, normalized: String): VoiceRuntimeResult =
        when {
            hasAny(normalized, "open", "openproject", "打开", "打开项目") -> VoiceRuntimeResult(
                state.copy(pageId = VoicePageId.PROJECT_SELECTION, selectedIndex = 0, notice = ""),
            )
            hasAny(normalized, "search", "搜索", "查找") -> VoiceRuntimeResult(
                state.copy(
                    pageId = VoicePageId.SEARCH_DICTATION,
                    inputMode = VoiceInputMode.DICTATION,
                    notice = "",
                ),
            )
            hasAny(normalized, "running", "运行中", "正在运行") -> {
                val running = state.tasks.filter { it.status == "running" }
                VoiceRuntimeResult(
                    state.copy(
                        pageId = VoicePageId.SEARCH_RESULTS,
                        searchQuery = "running",
                        searchResults = running,
                        selectedIndex = 0,
                        notice = if (running.isEmpty()) "No running projects." else "",
                    ),
                )
            }
            else -> VoiceRuntimeResult(state.copy(notice = "Command not available here."))
        }

    private fun handleSelection(
        state: VoiceRuntimeState,
        candidates: List<RemoteTaskSummary>,
        normalized: String,
    ): VoiceRuntimeResult {
        if (candidates.isEmpty()) {
            return VoiceRuntimeResult(state.copy(notice = "No projects available."))
        }
        if (hasAny(normalized, "next", "下一项", "下一个", "下一页", "更多")) {
            return VoiceRuntimeResult(state.copy(selectedIndex = (state.selectedIndex + 1).coerceAtMost(candidates.lastIndex)))
        }
        if (hasAny(normalized, "previous", "上一项", "上一个")) {
            return VoiceRuntimeResult(state.copy(selectedIndex = (state.selectedIndex - 1).coerceAtLeast(0)))
        }
        if (hasAny(normalized, "selectthis", "选择这个", "打开这个")) {
            return openCandidate(state, candidates[state.selectedIndex.coerceIn(candidates.indices)])
        }
        parseIndex(normalized)?.let { index ->
            candidates.getOrNull(index)?.let { return openCandidate(state, it) }
        }
        findTaskBySpokenName(candidates, normalized)?.let {
            return openCandidate(state, it)
        }
        return VoiceRuntimeResult(state.copy(notice = "Project not found."))
    }

    private fun openCandidate(state: VoiceRuntimeState, task: RemoteTaskSummary): VoiceRuntimeResult =
        VoiceRuntimeResult(
            state.copy(notice = "Opening ${task.title}."),
            VoiceAction.OpenTask(task.taskId),
        )

    private fun handleProjectDetail(
        state: VoiceRuntimeState,
        transcript: String,
        normalized: String,
    ): VoiceRuntimeResult {
        val taskId = state.selectedTask?.task?.taskId
            ?: return VoiceRuntimeResult(state.copy(notice = "No project selected."))
        return when {
            hasAny(normalized, "logs", "日志", "查看日志") -> VoiceRuntimeResult(
                state.copy(pageId = VoicePageId.LOG, notice = ""),
            )
            hasAny(normalized, "refresh", "刷新") -> VoiceRuntimeResult(
                state.copy(notice = "Refreshing."),
                VoiceAction.RefreshInbox,
            )
            startsWithAny(normalized, "start", "启动", "运行") -> {
                val command = startCommandFromTranscript(transcript)
                if (command.isBlank()) {
                    VoiceRuntimeResult(
                        state.copy(
                            pageId = VoicePageId.START_COMMAND_DICTATION,
                            inputMode = VoiceInputMode.DICTATION,
                            notice = "",
                        ),
                    )
                } else {
                    VoiceRuntimeResult(
                        state.copy(notice = "Starting process."),
                        VoiceAction.StartProcess(taskId, command),
                    )
                }
            }
            hasAny(normalized, "stop", "停止") -> VoiceRuntimeResult(
                state.copy(
                    pageId = VoicePageId.CONFIRM,
                    pendingConfirmation = VoiceConfirmation(
                        message = "Stop current process?",
                        action = VoiceAction.StopProcess(taskId),
                        returnPage = VoicePageId.PROJECT_DETAIL,
                    ),
                    notice = "Confirm stop.",
                ),
            )
            else -> VoiceRuntimeResult(state.copy(notice = "Command not available for this project."))
        }
    }

    private fun handleLog(state: VoiceRuntimeState, normalized: String): VoiceRuntimeResult =
        when {
            hasAny(normalized, "pause", "暂停") -> VoiceRuntimeResult(
                state.copy(logPaused = true, notice = "Log updates paused."),
            )
            hasAny(normalized, "continue", "resume", "继续") -> VoiceRuntimeResult(
                state.copy(logPaused = false, notice = "Log updates resumed."),
            )
            hasAny(normalized, "errors", "错误", "筛选错误") -> VoiceRuntimeResult(
                state.copy(logErrorsOnly = true, notice = "Showing errors only."),
            )
            hasAny(normalized, "summary", "总结") -> VoiceRuntimeResult(
                state.copy(notice = summarizeLog(state.selectedTask)),
            )
            else -> VoiceRuntimeResult(state.copy(notice = "Command not available for logs."))
        }

    private fun handleDictation(state: VoiceRuntimeState, text: String): VoiceRuntimeResult =
        when (state.pageId) {
            VoicePageId.SEARCH_DICTATION -> {
                val results = searchTasks(state.tasks, text)
                VoiceRuntimeResult(
                    state.copy(
                        pageId = VoicePageId.SEARCH_RESULTS,
                        inputMode = VoiceInputMode.COMMAND,
                        searchQuery = text,
                        searchResults = results,
                        selectedIndex = 0,
                        notice = if (results.isEmpty()) "No matching projects." else "",
                    ),
                )
            }
            VoicePageId.START_COMMAND_DICTATION -> {
                val taskId = state.selectedTask?.task?.taskId
                    ?: return VoiceRuntimeResult(
                        state.copy(pageId = VoicePageId.HUB, inputMode = VoiceInputMode.COMMAND),
                    )
                VoiceRuntimeResult(
                    state.copy(
                        pageId = VoicePageId.PROJECT_DETAIL,
                        inputMode = VoiceInputMode.COMMAND,
                        notice = "Starting process.",
                    ),
                    VoiceAction.StartProcess(taskId, text.trim()),
                )
            }
            else -> VoiceRuntimeResult(state.copy(inputMode = VoiceInputMode.COMMAND))
        }

    private fun handleGlobalCommand(state: VoiceRuntimeState, normalized: String): VoiceRuntimeResult? {
        if (isCancel(normalized) || hasAny(normalized, "back", "返回")) {
            return when (state.pageId) {
                VoicePageId.HUB -> VoiceRuntimeResult(state.copy(notice = "Already home."))
                VoicePageId.LOG -> VoiceRuntimeResult(state.copy(pageId = VoicePageId.PROJECT_DETAIL, notice = ""))
                VoicePageId.PROJECT_DETAIL -> VoiceRuntimeResult(state.copy(pageId = VoicePageId.HUB, notice = ""))
                else -> VoiceRuntimeResult(
                    state.copy(
                        pageId = VoicePageId.HUB,
                        inputMode = VoiceInputMode.COMMAND,
                        pendingConfirmation = null,
                        notice = "",
                    ),
                )
            }
        }
        if (hasAny(normalized, "home", "首页", "主页")) {
            return VoiceRuntimeResult(
                state.copy(
                    pageId = VoicePageId.HUB,
                    inputMode = VoiceInputMode.COMMAND,
                    pendingConfirmation = null,
                    notice = "",
                ),
            )
        }
        if (hasAny(normalized, "search", "搜索", "查找") && state.pageId != VoicePageId.SEARCH_DICTATION) {
            return VoiceRuntimeResult(
                state.copy(
                    pageId = VoicePageId.SEARCH_DICTATION,
                    inputMode = VoiceInputMode.DICTATION,
                    notice = "",
                ),
            )
        }
        return null
    }

    private fun selectionCommands(): List<VoiceCommand> =
        listOf(
            VoiceCommand("open_project", listOf("open {project}", "打开 {project}", "选择 {project}")),
            VoiceCommand("open_project_by_index", listOf("open first", "打开第一个", "选择第一个")),
            VoiceCommand("next_item", listOf("next", "下一项", "下一个")),
            VoiceCommand("previous_item", listOf("previous", "上一项", "上一个")),
        ) + globalCommands()

    private fun globalCommands(): List<VoiceCommand> =
        listOf(
            VoiceCommand("go_back", listOf("back", "cancel", "返回", "取消")),
            VoiceCommand("go_home", listOf("home", "首页", "主页")),
        )
}

private fun taskEntities(tasks: List<RemoteTaskSummary>): List<VoiceEntity> =
    tasks.map { task ->
        VoiceEntity(
            id = task.taskId,
            label = task.title,
            aliases = listOfNotNull(task.projectName),
        )
    }

private fun searchTasks(tasks: List<RemoteTaskSummary>, query: String): List<RemoteTaskSummary> {
    val needle = normalize(query)
    if (needle.isBlank()) return tasks
    return tasks.filter { task ->
        normalize(task.title).contains(needle) ||
            normalize(task.projectName.orEmpty()).contains(needle) ||
            normalize(task.status).contains(needle)
    }
}

private fun findTaskBySpokenName(tasks: List<RemoteTaskSummary>, normalized: String): RemoteTaskSummary? =
    tasks.firstOrNull { task ->
        val title = normalize(task.title)
        val project = normalize(task.projectName.orEmpty())
        title.isNotBlank() && normalized.contains(title) ||
            project.isNotBlank() && normalized.contains(project)
    }

private fun parseIndex(normalized: String): Int? {
    val words = mapOf(
        "first" to 0,
        "second" to 1,
        "third" to 2,
        "fourth" to 3,
        "fifth" to 4,
        "第一个" to 0,
        "第二个" to 1,
        "第三个" to 2,
        "第四个" to 3,
        "第五个" to 4,
        "一" to 0,
        "二" to 1,
        "三" to 2,
        "四" to 3,
        "五" to 4,
    )
    words.forEach { (word, index) ->
        if (normalized.contains(word)) return index
    }
    val number = Regex("\\d+").find(normalized)?.value?.toIntOrNull() ?: return null
    return (number - 1).coerceAtLeast(0)
}

private fun startCommandFromTranscript(transcript: String): String {
    val trimmed = transcript.trim()
    val prefixes = listOf("启动", "运行", "start", "run")
    val prefix = prefixes.firstOrNull { trimmed.startsWith(it, ignoreCase = true) } ?: return ""
    return trimmed.removePrefix(prefix).trim()
}

private fun summarizeLog(detail: RemoteTaskDetail?): String {
    val timeline = detail?.timeline.orEmpty()
    if (timeline.isEmpty()) return "No log events yet."
    val failed = timeline.count { item ->
        val status = normalize(item.status)
        status.contains("fail") || status.contains("error") || status.contains("cancel")
    }
    return "${timeline.size} events, $failed need attention."
}

private fun hasAny(normalized: String, vararg tokens: String): Boolean =
    tokens.any { normalized.contains(normalize(it)) }

private fun startsWithAny(normalized: String, vararg tokens: String): Boolean =
    tokens.any { normalized.startsWith(normalize(it)) }

private fun isConfirm(normalized: String): Boolean =
    hasAny(normalized, "confirm", "yes", "确认", "执行", "是")

private fun isCancel(normalized: String): Boolean =
    hasAny(normalized, "cancel", "no", "取消", "不要", "否")

private fun normalize(value: String): String =
    value.lowercase().filterNot { it.isWhitespace() || it == '，' || it == ',' || it == '.' || it == '。' }
