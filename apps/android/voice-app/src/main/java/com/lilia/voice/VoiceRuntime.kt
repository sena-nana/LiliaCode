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
                title = "首页",
                focus = VoiceFocus("hub", "项目入口"),
                commands = listOf(
                    VoiceCommand("open_project_selector", listOf("打开", "打开项目", "open", "open project")),
                    VoiceCommand("open_search", listOf("搜索", "查找", "search")),
                    VoiceCommand("show_running", listOf("运行中", "查看运行中", "running")),
                ),
            )
            VoicePageId.PROJECT_SELECTION -> VoicePageManifest(
                pageId = VoicePageId.PROJECT_SELECTION,
                title = "选择项目",
                focus = VoiceFocus("list", "项目列表"),
                commands = selectionCommands(),
                entities = taskEntities(state.tasks),
            )
            VoicePageId.SEARCH_DICTATION -> VoicePageManifest(
                pageId = VoicePageId.SEARCH_DICTATION,
                title = "搜索项目",
                focus = VoiceFocus("dictation", "搜索内容"),
                commands = globalCommands(),
            )
            VoicePageId.SEARCH_RESULTS -> VoicePageManifest(
                pageId = VoicePageId.SEARCH_RESULTS,
                title = "搜索结果",
                focus = VoiceFocus("list", "匹配项目"),
                commands = selectionCommands(),
                entities = taskEntities(state.searchResults),
            )
            VoicePageId.PROJECT_DETAIL -> VoicePageManifest(
                pageId = VoicePageId.PROJECT_DETAIL,
                title = state.selectedTask?.task?.title ?: "项目",
                focus = VoiceFocus("object", state.selectedTask?.task?.title ?: "当前项目"),
                commands = listOf(
                    VoiceCommand("start_project", listOf("启动", "运行", "start")),
                    VoiceCommand("stop_project", listOf("停止", "stop"), VoiceRisk.CONFIRM),
                    VoiceCommand("show_logs", listOf("查看日志", "日志", "logs")),
                    VoiceCommand("refresh", listOf("刷新", "refresh")),
                ) + globalCommands(),
            )
            VoicePageId.LOG -> VoicePageManifest(
                pageId = VoicePageId.LOG,
                title = "日志",
                focus = VoiceFocus("stream", "当前日志流"),
                commands = listOf(
                    VoiceCommand("pause_log", listOf("暂停", "pause")),
                    VoiceCommand("resume_log", listOf("继续", "continue")),
                    VoiceCommand("filter_errors", listOf("错误", "筛选错误", "errors")),
                    VoiceCommand("summarize_log", listOf("总结", "summary")),
                ) + globalCommands(),
            )
            VoicePageId.START_COMMAND_DICTATION -> VoicePageManifest(
                pageId = VoicePageId.START_COMMAND_DICTATION,
                title = "启动命令",
                focus = VoiceFocus("dictation", "进程命令"),
                commands = globalCommands(),
            )
            VoicePageId.CONFIRM -> VoicePageManifest(
                pageId = VoicePageId.CONFIRM,
                title = "确认",
                focus = VoiceFocus("confirm", "待确认操作"),
                commands = listOf(
                    VoiceCommand("confirm", listOf("确认", "执行", "是", "confirm", "yes")),
                    VoiceCommand("cancel_confirm", listOf("取消", "不要", "否", "cancel", "no")),
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
                        notice = "已取消。",
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
            -> VoiceRuntimeResult(state.copy(notice = "正在等待输入。"))
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
                        searchQuery = "运行中",
                        searchResults = running,
                        selectedIndex = 0,
                        notice = if (running.isEmpty()) "没有运行中的项目。" else "",
                    ),
                )
            }
            else -> VoiceRuntimeResult(state.copy(notice = "此处不能使用该命令。"))
        }

    private fun handleSelection(
        state: VoiceRuntimeState,
        candidates: List<RemoteTaskSummary>,
        normalized: String,
    ): VoiceRuntimeResult {
        if (candidates.isEmpty()) {
            return VoiceRuntimeResult(state.copy(notice = "没有可用项目。"))
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
        return VoiceRuntimeResult(state.copy(notice = "未找到项目。"))
    }

    private fun openCandidate(state: VoiceRuntimeState, task: RemoteTaskSummary): VoiceRuntimeResult =
        VoiceRuntimeResult(
            state.copy(notice = "正在打开 ${task.title}。"),
            VoiceAction.OpenTask(task.taskId),
        )

    private fun handleProjectDetail(
        state: VoiceRuntimeState,
        transcript: String,
        normalized: String,
    ): VoiceRuntimeResult {
        val taskId = state.selectedTask?.task?.taskId
            ?: return VoiceRuntimeResult(state.copy(notice = "尚未选择项目。"))
        return when {
            hasAny(normalized, "logs", "日志", "查看日志") -> VoiceRuntimeResult(
                state.copy(pageId = VoicePageId.LOG, notice = ""),
            )
            hasAny(normalized, "refresh", "刷新") -> VoiceRuntimeResult(
                state.copy(notice = "正在刷新。"),
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
                        state.copy(notice = "正在启动进程。"),
                        VoiceAction.StartProcess(taskId, command),
                    )
                }
            }
            hasAny(normalized, "stop", "停止") -> VoiceRuntimeResult(
                state.copy(
                    pageId = VoicePageId.CONFIRM,
                    pendingConfirmation = VoiceConfirmation(
                        message = "停止当前进程？",
                        action = VoiceAction.StopProcess(taskId),
                        returnPage = VoicePageId.PROJECT_DETAIL,
                    ),
                    notice = "请确认停止。",
                ),
            )
            else -> VoiceRuntimeResult(state.copy(notice = "此项目不能使用该命令。"))
        }
    }

    private fun handleLog(state: VoiceRuntimeState, normalized: String): VoiceRuntimeResult =
        when {
            hasAny(normalized, "pause", "暂停") -> VoiceRuntimeResult(
                state.copy(logPaused = true, notice = "已暂停日志更新。"),
            )
            hasAny(normalized, "continue", "resume", "继续") -> VoiceRuntimeResult(
                state.copy(logPaused = false, notice = "已恢复日志更新。"),
            )
            hasAny(normalized, "errors", "错误", "筛选错误") -> VoiceRuntimeResult(
                state.copy(logErrorsOnly = true, notice = "仅显示错误。"),
            )
            hasAny(normalized, "summary", "总结") -> VoiceRuntimeResult(
                state.copy(notice = summarizeLog(state.selectedTask)),
            )
            else -> VoiceRuntimeResult(state.copy(notice = "日志页不能使用该命令。"))
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
                        notice = if (results.isEmpty()) "没有匹配的项目。" else "",
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
                        notice = "正在启动进程。",
                    ),
                    VoiceAction.StartProcess(taskId, text.trim()),
                )
            }
            else -> VoiceRuntimeResult(state.copy(inputMode = VoiceInputMode.COMMAND))
        }

    private fun handleGlobalCommand(state: VoiceRuntimeState, normalized: String): VoiceRuntimeResult? {
        if (isCancel(normalized) || hasAny(normalized, "back", "返回")) {
            return when (state.pageId) {
                VoicePageId.HUB -> VoiceRuntimeResult(state.copy(notice = "已经在首页。"))
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
            VoiceCommand("open_project", listOf("打开 {project}", "选择 {project}", "open {project}")),
            VoiceCommand("open_project_by_index", listOf("打开第一个", "选择第一个", "open first")),
            VoiceCommand("next_item", listOf("下一项", "下一个", "next")),
            VoiceCommand("previous_item", listOf("上一项", "上一个", "previous")),
        ) + globalCommands()

    private fun globalCommands(): List<VoiceCommand> =
        listOf(
            VoiceCommand("go_back", listOf("返回", "取消", "back", "cancel")),
            VoiceCommand("go_home", listOf("首页", "主页", "home")),
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
    if (timeline.isEmpty()) return "还没有日志事件。"
    val failed = timeline.count { item ->
        val status = normalize(item.status)
        status.contains("fail") || status.contains("error") || status.contains("cancel")
    }
    return "${timeline.size} 个事件，$failed 个需要关注。"
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
