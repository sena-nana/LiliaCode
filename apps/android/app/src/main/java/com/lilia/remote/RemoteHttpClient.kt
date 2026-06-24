package com.lilia.remote

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

class RemoteHttpClient(
    private val repository: RemoteDeviceStore,
) {
    suspend fun bridgeStatus(pc: SavedPc): Result<RemoteBridgeStatus> = withContext(Dispatchers.IO) {
        runCatching {
            RemotePayloadParser.parseBridgeStatus(getJson("${bridgeUrl(pc)}/status"))
        }
    }

    suspend fun pair(ticket: RemotePairingTicket): Result<SavedPc> = withContext(Dispatchers.IO) {
        runCatching {
            val body = JSONObject()
                .put("ticketId", ticket.ticketId)
                .put("challenge", ticket.challenge)
                .put("deviceName", android.os.Build.MODEL ?: "Android device")
                .put("protocolVersion", ticket.protocolVersion)
                .put(
                    "androidEndpoint",
                    JSONObject()
                        .put("endpointId", repository.deviceEndpointId())
                        .put("relayUrl", JSONObject.NULL)
                        .put("directAddresses", JSONArray()),
                )
            val response = postJson("${bridgeUrl(ticket)}/pair", body)
            RemoteEnvelopeAdapter.throwIfError(response, "Pairing failed")
            repository.savePairing(ticket)
        }
    }

    suspend fun listTasks(pc: SavedPc): Result<List<RemoteTaskSummary>> = withContext(Dispatchers.IO) {
        runCatching {
            val payload = dispatch(pc, JSONObject().put("type", "tasks.list").put("limit", 80))
            RemotePayloadParser.parseTaskList(payload)
        }
    }

    suspend fun providerStatus(pc: SavedPc): Result<RemoteProviderStatus> = withContext(Dispatchers.IO) {
        runCatching {
            val payload = dispatch(pc, JSONObject().put("type", "provider.status.read"))
            RemotePayloadParser.parseProviderStatus(payload)
        }
    }

    suspend fun resume(pc: SavedPc): Result<Boolean> = withContext(Dispatchers.IO) {
        runCatching {
            val payload = dispatch(
                pc,
                JSONObject()
                    .put("type", "connection.resume")
                    .put("androidEndpointId", repository.deviceEndpointId()),
            )
            payload.optBoolean("accepted")
        }
    }

    suspend fun taskDetail(pc: SavedPc, taskId: String): Result<RemoteTaskDetail> = withContext(Dispatchers.IO) {
        runCatching {
            val taskPayload = dispatch(pc, JSONObject().put("type", "tasks.get").put("taskId", taskId))
            val timelinePayload = dispatch(pc, JSONObject().put("type", "timeline.snapshot").put("taskId", taskId))
            val pendingPayload = dispatch(pc, JSONObject().put("type", "interaction.pending.read").put("taskId", taskId))
            RemotePayloadParser.parseTaskDetail(taskId, taskPayload, timelinePayload, pendingPayload)
        }
    }

    suspend fun taskState(pc: SavedPc, taskId: String): Result<RemoteTaskState> = withContext(Dispatchers.IO) {
        runCatching {
            val taskPayload = dispatch(pc, JSONObject().put("type", "tasks.get").put("taskId", taskId))
            val pendingPayload = dispatch(pc, JSONObject().put("type", "interaction.pending.read").put("taskId", taskId))
            RemotePayloadParser.parseTaskState(taskId, taskPayload, pendingPayload)
        }
    }

    suspend fun subscribeTimeline(
        pc: SavedPc,
        taskId: String,
        afterEventId: String?,
    ): Result<List<RemoteTimelineItem>> = withContext(Dispatchers.IO) {
        runCatching {
            val request = JSONObject()
                .put("type", "timeline.subscribe")
                .put("taskId", taskId)
            if (afterEventId != null) {
                request.put("afterEventId", afterEventId)
            }
            val payload = dispatch(pc, request)
            RemotePayloadParser.parseTimelinePayload(payload)
        }
    }

    suspend fun sendMessage(
        pc: SavedPc,
        input: RemoteSendMessageInput,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            dispatch(pc, input.toRequestJson())
            Unit
        }
    }

    suspend fun interrupt(pc: SavedPc, taskId: String): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            dispatch(pc, JSONObject().put("type", "chat.interrupt").put("taskId", taskId))
            Unit
        }
    }

    suspend fun retry(
        pc: SavedPc,
        taskId: String,
        eventId: String? = null,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val request = JSONObject().put("type", "chat.retry").put("taskId", taskId)
            if (!eventId.isNullOrBlank()) {
                request.put("eventId", eventId)
            }
            dispatch(pc, request)
            Unit
        }
    }

    suspend fun resolveInteraction(
        pc: SavedPc,
        interaction: PendingInteraction,
        approve: Boolean,
        responseText: String? = null,
        selectedOptionIdsByQuestion: Map<String, List<String>> = emptyMap(),
    ): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val response = JSONObject()
                .put("taskId", interaction.taskId)
                .put("requestId", interaction.requestId)
                .put("kind", interaction.kind)
                .put(
                    "result",
                    RemoteInteractionAdapter.resultForInteraction(
                        interaction,
                        approve,
                        responseText,
                        selectedOptionIdsByQuestion,
                    ),
                )
            dispatch(pc, JSONObject().put("type", "interaction.respond").put("response", response))
            Unit
        }
    }

    private fun dispatch(pc: SavedPc, request: JSONObject): JSONObject {
        val envelope = RemoteEnvelopeAdapter.requestEnvelope(pc, repository.deviceEndpointId(), request)
        val response = postJson("${bridgeUrl(pc)}/dispatch", envelope)
        val payload = RemoteEnvelopeAdapter.payloadOrThrow(response)
        if (shouldMarkActivePcSeen(request, payload)) {
            repository.markActivePcSeen(pc)
        }
        return payload
    }

    private fun bridgeUrl(pc: SavedPc): String = pc.bridgeUrl.trim().trimEnd('/')

    private fun bridgeUrl(ticket: RemotePairingTicket): String = ticket.bridgeUrl.trim().trimEnd('/')

    private fun shouldMarkActivePcSeen(request: JSONObject, payload: JSONObject): Boolean {
        if (request.optString("type") != "connection.resume") return true
        return payload.optBoolean("accepted")
    }

    private fun postJson(url: String, body: JSONObject): JSONObject {
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            connectTimeout = 5_000
            readTimeout = 30_000
            doOutput = true
            setRequestProperty("Content-Type", "application/json; charset=utf-8")
        }
        OutputStreamWriter(connection.outputStream, Charsets.UTF_8).use { writer ->
            writer.write(body.toString())
        }
        return try {
            readJsonResponse(connection)
        } finally {
            connection.disconnect()
        }
    }

    private fun getJson(url: String): JSONObject {
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            requestMethod = "GET"
            connectTimeout = 5_000
            readTimeout = 10_000
        }
        return try {
            readJsonResponse(connection)
        } finally {
            connection.disconnect()
        }
    }

    private fun readJsonResponse(connection: HttpURLConnection): JSONObject {
        val statusCode = connection.responseCode
        val stream = if (statusCode in 200..299) {
            connection.inputStream
        } else {
            connection.errorStream
        }
        val text = stream?.let {
            BufferedReader(it.reader(Charsets.UTF_8)).use { reader -> reader.readText() }
        }.orEmpty()
        return RemoteHttpResponseAdapter.parseJson(statusCode, text)
    }
}

private fun RemoteSendMessageInput.toRequestJson(): JSONObject =
    JSONObject()
        .put("type", "chat.send")
        .put("taskId", taskId)
        .put("content", content)
        .putIfPresent("composer", composer)
        .putIfPresent("attachments", attachments)
        .putIfPresent("conversationReferences", conversationReferences)
        .putIfPresent("workflow", workflow)
        .putIfPresent("runtimeCommand", runtimeCommand)
        .putIfPresent("runtimeOptions", runtimeOptions)

private fun JSONObject.putIfPresent(key: String, value: Any?): JSONObject =
    if (value == null) this else put(key, value)
