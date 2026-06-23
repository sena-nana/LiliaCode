package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.json.JSONObject
import org.junit.Test

class MainActivityStateTest {
    @Test
    fun savedPcConnectionMatchesRequiresSameEndpointAndBridge() {
        val pc = savedPc(endpointId = "pc-1", bridgeUrl = "http://192.168.1.12:41478")

        assertTrue(savedPcConnectionMatches(pc, pc.copy(displayName = "Renamed")))
        assertFalse(savedPcConnectionMatches(null, pc))
        assertFalse(savedPcConnectionMatches(pc.copy(endpointId = "pc-2"), pc))
        assertFalse(savedPcConnectionMatches(pc.copy(bridgeUrl = "http://192.168.1.13:41478"), pc))
    }

    @Test
    fun shouldIgnorePcResultIgnoresClearedOrStaleActivePcResults() {
        val pc = savedPc(endpointId = "pc-1", bridgeUrl = "http://192.168.1.12:41478")

        assertTrue(shouldIgnorePcResult(null, pc))
        assertFalse(shouldIgnorePcResult(pc, pc.copy(displayName = "Renamed")))
        assertTrue(shouldIgnorePcResult(pc.copy(endpointId = "pc-2"), pc))
        assertTrue(shouldIgnorePcResult(pc.copy(bridgeUrl = "http://192.168.1.13:41478"), pc))
    }

    @Test
    fun switchingActivePcResetsDerivedStateAndMakesOldResultsStale() {
        val oldPc = savedPc(endpointId = "pc-1", bridgeUrl = "http://192.168.1.12:41478")
        val newPc = savedPc(endpointId = "pc-2", bridgeUrl = "http://192.168.1.13:41478")

        assertTrue(shouldIgnorePcResult(newPc, oldPc))
        assertFalse(shouldIgnorePcResult(newPc, newPc))
    }

    @Test
    fun unauthorizedRemoteFailureRequiresPairAgain() {
        val revoked = RemoteBridgeException(
            code = "unauthorized",
            message = "Android device is no longer trusted",
        )
        val offline = RemoteBridgeException(
            code = "unavailable",
            message = "Bridge offline",
            retryable = true,
        )

        assertTrue(shouldClearActivePcForFailure(revoked))
        assertFalse(shouldClearActivePcForFailure(offline))
        assertEquals("This PC no longer trusts this phone. Pair again.", UNTRUSTED_PC_MESSAGE)
    }

    @Test
    fun sessionForkRuntimeCommandUsesDesktopWireShape() {
        val command = RemoteRuntimeCommandAdapter.sessionFork(
            RemoteBranchAnchor("turn-source", RemoteSessionForkMode.CONTINUE),
        )

        assertSessionForkCommand(command, "turn-source", "continue")
    }

    @Test
    fun syncInboxTaskStatusUpdatesMatchingCardStatusAndPendingAction() {
        val tasks = listOf(
            taskSummary(status = "running"),
            taskSummary(taskId = "task-2", status = "waiting"),
        )
        val detail = taskDetail(status = "blocked", pendingActionTitle = "Permission request")

        val updated = syncInboxTaskStatus(tasks, detail)

        assertEquals("blocked", updated[0].status)
        assertEquals("Permission request", updated[0].pendingAction)
        assertEquals("waiting", updated[1].status)
        assertEquals(null, updated[1].pendingAction)
    }

    @Test
    fun syncInboxTaskStatusClearsResolvedPendingAction() {
        val tasks = listOf(taskSummary(status = "blocked", pendingAction = "Permission request"))
        val detail = taskDetail(status = "running")

        val updated = syncInboxTaskStatus(tasks, detail)

        assertEquals("running", updated[0].status)
        assertEquals(null, updated[0].pendingAction)
    }

    private fun savedPc(endpointId: String, bridgeUrl: String): SavedPc =
        SavedPc(
            endpointId = endpointId,
            displayName = "Desk",
            protocolVersion = 1,
            pairingUri = "lilia-remote://pair?v=1",
            bridgeUrl = bridgeUrl,
            lastActiveAt = 1710000000000,
        )

    private fun taskSummary(
        taskId: String = "task-1",
        status: String = "running",
        pendingAction: String? = null,
    ): RemoteTaskSummary =
        RemoteTaskSummary(
            taskId = taskId,
            title = "Task $taskId",
            projectName = "Lilia",
            status = status,
            lastActivity = "Today",
            pendingAction = pendingAction,
        )

    private fun taskDetail(
        taskId: String = "task-1",
        status: String = "running",
        pendingActionTitle: String? = null,
    ): RemoteTaskDetail =
        RemoteTaskDetail(
            task = taskSummary(taskId = taskId, status = status),
            runtimePhase = status,
            timeline = emptyList(),
            pendingInteraction = pendingActionTitle?.let(::pendingInteraction),
        )

    private fun pendingInteraction(title: String): PendingInteraction =
        PendingInteraction(
            taskId = "task-1",
            requestId = "request-1",
            kind = "tool_consent",
            backend = "codex",
            title = title,
            body = "Allow command?",
            approveLabel = "Allow",
            declineLabel = "Deny",
            raw = JSONObject(),
        )

    private fun assertSessionForkCommand(command: org.json.JSONObject, sourceTurnId: String, mode: String) {
        assertEquals("session_fork", command.getString("type"))
        assertEquals(true, command.getBoolean("excludeTurns"))
        assertEquals(sourceTurnId, command.getString("sourceTurnId"))
        assertEquals(mode, command.getString("mode"))
    }
}
