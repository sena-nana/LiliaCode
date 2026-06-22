package com.lilia.remote

import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.net.InetSocketAddress
import java.util.concurrent.atomic.AtomicReference

class RemoteHttpClientTest {
    private val servers = mutableListOf<HttpServer>()

    @After
    fun stopServers() {
        servers.forEach { it.stop(0) }
        servers.clear()
    }

    @Test
    fun bridgeStatusReadsLanBridgeStatus() = runBlocking {
        val server = startBridge { exchange ->
            assertEquals("GET", exchange.requestMethod)
            assertEquals("/status", exchange.requestURI.path)
            exchange.respond(
                """
                {
                  "ok": true,
                  "status": {
                    "hostEnabled": true,
                    "state": "listening",
                    "pcName": "Desk"
                  }
                }
                """.trimIndent(),
            )
        }

        val status = RemoteHttpClient(FakeDeviceStore())
            .bridgeStatus(savedPc(server))
            .getOrThrow()

        assertEquals(true, status.hostEnabled)
        assertEquals("listening", status.state)
        assertEquals("Desk", status.pcName)
    }

    @Test
    fun pairPostsTicketAndStableAndroidEndpoint() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/pair", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond("""{ "ok": true, "peer": { "id": "peer-1" } }""")
        }
        val ticket = RemotePairingTicket(
            protocolVersion = 1,
            ticketId = "ticket-1",
            challenge = "challenge-1",
            endpointId = "pc-endpoint",
            pcName = "Desk",
            bridgeUrl = bridgeUrl(server),
            rawUri = "lilia-remote://pair?v=1",
        )

        val pc = RemoteHttpClient(FakeDeviceStore())
            .pair(ticket)
            .getOrThrow()

        val body = requestBody.get()
        assertNotNull(body)
        assertEquals("ticket-1", body.getString("ticketId"))
        assertEquals("challenge-1", body.getString("challenge"))
        assertEquals("android-device-1", body.getJSONObject("androidEndpoint").getString("endpointId"))
        assertEquals("pc-endpoint", pc.endpointId)
    }

    @Test
    fun pairPreservesStructuredBridgeErrorCode() = runBlocking {
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/pair", exchange.requestURI.path)
            exchange.respond(
                """
                {
                  "ok": false,
                  "error": {
                    "code": "invalidRequest",
                    "message": "pairing ticket expired",
                    "retryable": false
                  }
                }
                """.trimIndent(),
            )
        }
        val ticket = RemotePairingTicket(
            protocolVersion = 1,
            ticketId = "ticket-1",
            challenge = "challenge-1",
            endpointId = "pc-endpoint",
            pcName = "Desk",
            bridgeUrl = bridgeUrl(server),
            rawUri = "lilia-remote://pair?v=1",
        )

        val err = RemoteHttpClient(FakeDeviceStore())
            .pair(ticket)
            .exceptionOrNull()

        assertTrue(err is RemoteBridgeException)
        assertEquals("invalidRequest", (err as RemoteBridgeException).code)
        assertEquals("pairing ticket expired", err.message)
    }

    @Test
    fun listTasksDispatchesTrustedDeviceEnvelope() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "tasks.list",
                    "tasks": [
                      {
                        "taskId": "task-1",
                        "title": "Remote task",
                        "projectName": "Lilia",
                        "status": "running",
                        "createdAt": 1710000000000
                      }
                    ]
                  }
                }
                """.trimIndent(),
            )
        }

        val store = FakeDeviceStore()
        val pc = savedPc(server)
        val tasks = RemoteHttpClient(store)
            .listTasks(pc)
            .getOrThrow()

        val envelope = requestBody.get()
        assertNotNull(envelope)
        assertEquals("android-device-1", envelope.getString("deviceId"))
        assertEquals("tasks.list", envelope.getJSONObject("request").getString("type"))
        assertEquals(1, tasks.size)
        assertEquals("Remote task", tasks[0].title)
        assertEquals("running", tasks[0].status)
        assertEquals(listOf(pc.endpointId), store.seenEndpointIds)
    }

    @Test
    fun resumeDispatchesStableAndroidEndpointId() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "connection.resume",
                    "accepted": true,
                    "peer": { "id": "peer-1" }
                  }
                }
                """.trimIndent(),
            )
        }

        val store = FakeDeviceStore()
        val pc = savedPc(server)
        val accepted = RemoteHttpClient(store)
            .resume(pc)
            .getOrThrow()

        val envelope = requestBody.get()
        val request = envelope.getJSONObject("request")
        assertEquals(true, accepted)
        assertEquals("android-device-1", envelope.getString("deviceId"))
        assertEquals("connection.resume", request.getString("type"))
        assertEquals("android-device-1", request.getString("androidEndpointId"))
        assertEquals(listOf(pc.endpointId), store.seenEndpointIds)
    }

    @Test
    fun resumeReturnsFalseWhenDesktopDoesNotAcceptDevice() = runBlocking {
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "connection.resume",
                    "accepted": false,
                    "peer": null
                  }
                }
                """.trimIndent(),
            )
        }

        val store = FakeDeviceStore()
        val accepted = RemoteHttpClient(store)
            .resume(savedPc(server))
            .getOrThrow()

        assertEquals(false, accepted)
        assertEquals(emptyList<String>(), store.seenEndpointIds)
    }

    @Test
    fun providerStatusDispatchesProviderStatusReadRequest() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "provider.status",
                    "backend": "codex",
                    "ready": false
                  }
                }
                """.trimIndent(),
            )
        }

        val status = RemoteHttpClient(FakeDeviceStore())
            .providerStatus(savedPc(server))
            .getOrThrow()

        val request = requestBody.get().getJSONObject("request")
        assertEquals("provider.status.read", request.getString("type"))
        assertEquals("codex", status.backend)
        assertEquals(false, status.ready)
    }

    @Test
    fun dispatchPreservesUnauthorizedErrorCode() = runBlocking {
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            exchange.respond(
                """
                {
                  "ok": false,
                  "error": {
                    "code": "unauthorized",
                    "message": "Android device is no longer trusted",
                    "retryable": false
                  }
                }
                """.trimIndent(),
            )
        }

        val err = RemoteHttpClient(FakeDeviceStore())
            .listTasks(savedPc(server))
            .exceptionOrNull()

        assertTrue(err is RemoteBridgeException)
        assertEquals("unauthorized", (err as RemoteBridgeException).code)
        assertEquals("Android device is no longer trusted", err.message)
    }

    @Test
    fun dispatchHandlesHttpErrorWithoutBodyAsEmptyResponse() = runBlocking {
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            exchange.respondEmpty(500)
        }

        val err = RemoteHttpClient(FakeDeviceStore())
            .listTasks(savedPc(server))
            .exceptionOrNull()

        assertEquals("Remote bridge returned an empty response (HTTP 500).", err?.message)
    }

    @Test
    fun taskDetailDispatchesTaskTimelineAndPendingRequests() = runBlocking {
        val requests = mutableListOf<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            val envelope = JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText())
            val request = envelope.getJSONObject("request")
            requests.add(request)
            val payload = when (request.getString("type")) {
                "tasks.get" -> """
                    {
                      "type": "tasks.get",
                      "task": {
                        "taskId": "task-1",
                        "title": "Remote task",
                        "status": "running",
                        "createdAt": 1710000000000
                      },
                      "runtime": { "phase": "running" }
                    }
                """.trimIndent()
                "timeline.snapshot" -> """
                    {
                      "type": "timeline.snapshot",
                      "taskId": "task-1",
                      "events": [
                        {
                          "id": "event-1",
                          "kind": "assistant_message",
                          "summary": "Working",
                          "status": "completed"
                        }
                      ]
                    }
                """.trimIndent()
                "interaction.pending.read" -> """
                    {
                      "type": "interaction.pending",
                      "interactions": []
                    }
                """.trimIndent()
                else -> error("unexpected request type ${request.getString("type")}")
            }
            exchange.respond("""{ "ok": true, "payload": $payload }""")
        }

        val detail = RemoteHttpClient(FakeDeviceStore())
            .taskDetail(savedPc(server), "task-1")
            .getOrThrow()

        assertEquals("task-1", detail.task.taskId)
        assertEquals("running", detail.runtimePhase)
        assertEquals(3, requests.size)
        assertEquals("tasks.get", requests[0].getString("type"))
        assertEquals("timeline.snapshot", requests[1].getString("type"))
        assertEquals("interaction.pending.read", requests[2].getString("type"))
        requests.forEach { request ->
            assertEquals("task-1", request.getString("taskId"))
        }
    }

    @Test
    fun sendMessageDispatchesChatSendRequest() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "chat.send",
                    "result": { "accepted": true }
                  }
                }
                """.trimIndent(),
            )
        }

        RemoteHttpClient(FakeDeviceStore())
            .sendMessage(savedPc(server), "task-1", "Continue from Android")
            .getOrThrow()

        val request = requestBody.get().getJSONObject("request")
        assertEquals("chat.send", request.getString("type"))
        assertEquals("task-1", request.getString("taskId"))
        assertEquals("Continue from Android", request.getString("content"))
        assertFalse(request.has("runtimeCommand"))
    }

    @Test
    fun sendMessageDispatchesRuntimeCommandWhenPresent() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "chat.send",
                    "result": { "accepted": true }
                  }
                }
                """.trimIndent(),
            )
        }
        val runtimeCommand = RemoteRuntimeCommandAdapter.sessionFork(
            RemoteBranchAnchor("turn-source", RemoteSessionForkMode.FORK),
        )

        RemoteHttpClient(FakeDeviceStore())
            .sendMessage(savedPc(server), "task-1", "Continue from Android", runtimeCommand)
            .getOrThrow()

        val command = requestBody.get()
            .getJSONObject("request")
            .getJSONObject("runtimeCommand")
        assertSessionForkCommand(command, "turn-source", "fork")
    }

    @Test
    fun interruptDispatchesChatInterruptRequest() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "chat.interrupt",
                    "result": { "interrupted": true }
                  }
                }
                """.trimIndent(),
            )
        }

        RemoteHttpClient(FakeDeviceStore())
            .interrupt(savedPc(server), "task-1")
            .getOrThrow()

        val request = requestBody.get().getJSONObject("request")
        assertEquals("chat.interrupt", request.getString("type"))
        assertEquals("task-1", request.getString("taskId"))
    }

    @Test
    fun resolveInteractionDispatchesStableTaskId() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "interaction.respond",
                    "accepted": true
                  }
                }
                """.trimIndent(),
            )
        }
        val interaction = PendingInteraction(
            taskId = "task-1",
            requestId = "request-1",
            kind = "tool_consent",
            backend = "codex",
            title = "Pending",
            body = "Body",
            approveLabel = "Approve",
            declineLabel = "Decline",
            raw = JSONObject()
                .put("requestId", "request-1")
                .put("kind", "tool_consent")
                .put("payload", JSONObject()),
        )

        RemoteHttpClient(FakeDeviceStore())
            .resolveInteraction(savedPc(server), interaction, approve = true)
            .getOrThrow()

        val request = requestBody.get().getJSONObject("request")
        val response = request.getJSONObject("response")
        val result = response.getJSONObject("result")
        assertEquals("interaction.respond", request.getString("type"))
        assertEquals("task-1", response.getString("taskId"))
        assertEquals("request-1", response.getString("requestId"))
        assertEquals("task-1", result.getString("taskId"))
        assertEquals("allow", result.getString("decision"))
    }

    @Test
    fun resolveInteractionDispatchesTypedAskUserAnswer() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "interaction.respond",
                    "accepted": true
                  }
                }
                """.trimIndent(),
            )
        }
        val interaction = PendingInteraction(
            taskId = "task-1",
            requestId = "request-1",
            kind = "ask_user",
            backend = "codex",
            title = "Question",
            body = "What next?",
            approveLabel = "Answer",
            declineLabel = "Decline",
            raw = JSONObject(
                """
                {
                  "requestId": "request-1",
                  "kind": "ask_user",
                  "payload": {
                    "questions": [
                      { "id": "reply", "question": "What next?" }
                    ]
                  }
                }
                """.trimIndent(),
            ),
        )

        RemoteHttpClient(FakeDeviceStore())
            .resolveInteraction(savedPc(server), interaction, approve = true, responseText = "Continue from Android")
            .getOrThrow()

        val result = requestBody.get()
            .getJSONObject("request")
            .getJSONObject("response")
            .getJSONObject("result")
        assertEquals(false, result.getBoolean("cancelled"))
        assertEquals(
            "Continue from Android",
            result.getJSONObject("answers").getJSONObject("reply").getString("value"),
        )
    }

    @Test
    fun resolveInteractionDispatchesSelectedAskUserOption() = runBlocking {
        val requestBody = AtomicReference<JSONObject>()
        val server = startBridge { exchange ->
            assertEquals("POST", exchange.requestMethod)
            assertEquals("/dispatch", exchange.requestURI.path)
            requestBody.set(JSONObject(exchange.requestBody.reader(Charsets.UTF_8).readText()))
            exchange.respond(
                """
                {
                  "ok": true,
                  "payload": {
                    "type": "interaction.respond",
                    "accepted": true
                  }
                }
                """.trimIndent(),
            )
        }
        val interaction = PendingInteraction(
            taskId = "task-1",
            requestId = "request-1",
            kind = "ask_user",
            backend = "codex",
            title = "Question",
            body = "Choose one",
            approveLabel = "Answer",
            declineLabel = "Decline",
            raw = JSONObject(
                """
                {
                  "requestId": "request-1",
                  "kind": "ask_user",
                  "payload": {
                    "questions": [
                      {
                        "id": "choice",
                        "mode": "single",
                        "options": [
                          { "id": "first" },
                          { "id": "second" }
                        ]
                      }
                    ]
                  }
                }
                """.trimIndent(),
            ),
        )

        RemoteHttpClient(FakeDeviceStore())
            .resolveInteraction(
                savedPc(server),
                interaction,
                approve = true,
                selectedOptionIdsByQuestion = mapOf("choice" to listOf("second")),
            )
            .getOrThrow()

        val result = requestBody.get()
            .getJSONObject("request")
            .getJSONObject("response")
            .getJSONObject("result")
        assertEquals("second", result.getJSONObject("answers").getJSONObject("choice").getString("value"))
    }

    private fun startBridge(handler: (HttpExchange) -> Unit): HttpServer {
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/") { exchange ->
            try {
                handler(exchange)
            } catch (err: Throwable) {
                exchange.respond("""{ "ok": false, "error": "${err.message}" }""", 500)
            }
        }
        server.start()
        servers.add(server)
        return server
    }

    private fun HttpExchange.respond(body: String, statusCode: Int = 200) {
        val bytes = body.toByteArray(Charsets.UTF_8)
        responseHeaders.add("Content-Type", "application/json; charset=utf-8")
        sendResponseHeaders(statusCode, bytes.size.toLong())
        responseBody.use { it.write(bytes) }
    }

    private fun HttpExchange.respondEmpty(statusCode: Int) {
        sendResponseHeaders(statusCode, -1)
        responseBody.close()
    }

    private fun savedPc(server: HttpServer): SavedPc = SavedPc(
        endpointId = "pc-endpoint",
        displayName = "Desk",
        protocolVersion = 1,
        pairingUri = "lilia-remote://pair?v=1",
        bridgeUrl = bridgeUrl(server),
        lastActiveAt = 1710000000000,
    )

    private fun bridgeUrl(server: HttpServer): String =
        "http://127.0.0.1:${server.address.port}"

    private fun assertSessionForkCommand(command: JSONObject, sourceTurnId: String, mode: String) {
        assertEquals("session_fork", command.getString("type"))
        assertEquals(true, command.getBoolean("excludeTurns"))
        assertEquals(sourceTurnId, command.getString("sourceTurnId"))
        assertEquals(mode, command.getString("mode"))
    }

    private class FakeDeviceStore : RemoteDeviceStore {
        val seenEndpointIds = mutableListOf<String>()

        override fun deviceEndpointId(): String = "android-device-1"

        override fun savePairing(ticket: RemotePairingTicket): SavedPc = SavedPc(
            endpointId = ticket.endpointId,
            displayName = ticket.pcName,
            protocolVersion = ticket.protocolVersion,
            pairingUri = ticket.rawUri,
            bridgeUrl = ticket.bridgeUrl,
            lastActiveAt = 1710000000000,
        )

        override fun markActivePcSeen(pc: SavedPc, now: Long): SavedPc {
            seenEndpointIds.add(pc.endpointId)
            return pc.copy(lastActiveAt = now)
        }
    }
}
