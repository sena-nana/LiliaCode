package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
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
    fun processSessionRuntimeCommandsUseDesktopWireShape() {
        val spawn = RemoteRuntimeCommandAdapter.processSpawn("npm test")
        val stdin = RemoteRuntimeCommandAdapter.processWriteStdin("q")
        val kill = RemoteRuntimeCommandAdapter.processKill()

        assertEquals("process_session", spawn.getString("type"))
        assertEquals("spawn", spawn.getString("action"))
        assertEquals("npm test", spawn.getString("command"))
        assertEquals("process_session", stdin.getString("type"))
        assertEquals("write_stdin", stdin.getString("action"))
        assertEquals("q", stdin.getString("stdin"))
        assertEquals("process_session", kill.getString("type"))
        assertEquals("kill", kill.getString("action"))
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

    private fun assertSessionForkCommand(command: org.json.JSONObject, sourceTurnId: String, mode: String) {
        assertEquals("session_fork", command.getString("type"))
        assertEquals(true, command.getBoolean("excludeTurns"))
        assertEquals(sourceTurnId, command.getString("sourceTurnId"))
        assertEquals(mode, command.getString("mode"))
    }
}
