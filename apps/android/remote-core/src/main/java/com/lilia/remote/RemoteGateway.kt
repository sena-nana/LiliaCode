package com.lilia.remote

import android.content.Context

class RemoteGateway(
    val repository: RemoteRepository,
    private val client: RemoteHttpClient = RemoteHttpClient(repository),
) {
    constructor(context: Context) : this(RemoteRepository(context))

    fun savedPcs(): List<SavedPc> = repository.savedPcs()

    fun activePc(): SavedPc? = repository.activePc()

    fun activeTaskId(): String? = repository.activeTaskId()

    fun setActiveTaskId(taskId: String?) {
        repository.setActiveTaskId(taskId)
    }

    fun clearActivePc() {
        repository.clearActivePc()
    }

    fun forgetPc(pc: SavedPc) {
        repository.forgetPc(pc)
    }

    fun switchActivePc(pc: SavedPc): SavedPc? =
        repository.switchActivePc(pc)

    suspend fun pair(ticket: RemotePairingTicket): Result<SavedPc> =
        client.pair(ticket)

    suspend fun resume(pc: SavedPc): Result<Boolean> =
        client.resume(pc)

    suspend fun bridgeStatus(pc: SavedPc): Result<RemoteBridgeStatus> =
        client.bridgeStatus(pc)

    suspend fun providerStatus(pc: SavedPc): Result<RemoteProviderStatus> =
        client.providerStatus(pc)

    suspend fun listTasks(pc: SavedPc): Result<List<RemoteTaskSummary>> =
        client.listTasks(pc)

    suspend fun taskDetail(pc: SavedPc, taskId: String): Result<RemoteTaskDetail> =
        client.taskDetail(pc, taskId)

    suspend fun timelineBefore(pc: SavedPc, taskId: String, beforeCursor: String): Result<RemoteTimelinePage> =
        client.timelineBefore(pc, taskId, beforeCursor)

    suspend fun taskState(pc: SavedPc, taskId: String): Result<RemoteTaskState> =
        client.taskState(pc, taskId)

    suspend fun subscribeTimeline(pc: SavedPc, taskId: String, afterEventId: String?): Result<List<RemoteTimelineItem>> =
        client.subscribeTimeline(pc, taskId, afterEventId)

    suspend fun sendMessage(pc: SavedPc, input: RemoteSendMessageInput): Result<Unit> =
        client.sendMessage(pc, input)

    suspend fun interrupt(pc: SavedPc, taskId: String): Result<Unit> =
        client.interrupt(pc, taskId)

    suspend fun retry(pc: SavedPc, taskId: String, eventId: String? = null): Result<Unit> =
        client.retry(pc, taskId, eventId)

    suspend fun resolveInteraction(
        pc: SavedPc,
        interaction: PendingInteraction,
        approve: Boolean,
        responseText: String? = null,
        selectedOptionIdsByQuestion: Map<String, List<String>> = emptyMap(),
    ): Result<Unit> =
        client.resolveInteraction(pc, interaction, approve, responseText, selectedOptionIdsByQuestion)

    suspend fun startProcess(pc: SavedPc, detail: RemoteTaskDetail, command: String): Result<Unit> {
        val blockInfo = taskRunBlockInfo(detail.task, detail.relatedTasks)
        if (blockInfo != null) {
            return Result.failure(IllegalStateException(blockInfo.reason))
        }
        return client.sendMessage(
            pc,
            RemoteSendMessageInput(
                taskId = detail.task.taskId,
                content = "",
                runtimeCommand = RemoteRuntimeCommandAdapter.processSpawn(command),
            ),
        )
    }

    suspend fun stopProcess(pc: SavedPc, taskId: String): Result<Unit> =
        client.sendMessage(
            pc,
            RemoteSendMessageInput(
                taskId = taskId,
                content = "",
                runtimeCommand = RemoteRuntimeCommandAdapter.processKill(),
            ),
        )
}
