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
    fun sessionForkRuntimeCommandUsesDesktopWireShape() {
        val command = RemoteRuntimeCommandAdapter.sessionFork(
            RemoteBranchAnchor("turn-source", RemoteSessionForkMode.CONTINUE),
        )

        assertSessionForkCommand(command, "turn-source", "continue")
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
