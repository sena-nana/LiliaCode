package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteRepositoryTest {
    @Test
    fun deviceEndpointIdIsStableAcrossRepositoryInstances() {
        val preferences = InMemoryRemotePreferences()
        val first = RemoteRepository(preferences).deviceEndpointId()
        val second = RemoteRepository(preferences).deviceEndpointId()

        assertTrue(first.startsWith("android-"))
        assertEquals(first, second)
    }

    @Test
    fun savePairingPersistsActivePcWithBridgeUrl() {
        val preferences = InMemoryRemotePreferences()
        val saved = RemoteRepository(preferences).savePairing(ticket())

        val restored = RemoteRepository(preferences).activePc()

        assertNotNull(restored)
        assertEquals("pc-endpoint", saved.endpointId)
        assertEquals("pc-endpoint", restored?.endpointId)
        assertEquals("Desk", restored?.displayName)
        assertEquals(1, restored?.protocolVersion)
        assertEquals("lilia-remote://pair?v=1", restored?.pairingUri)
        assertEquals("http://192.168.1.12:41478", restored?.bridgeUrl)
        assertTrue((restored?.lastActiveAt ?: 0L) > 0L)
    }

    @Test
    fun clearActivePcRemovesPairingStateButKeepsDeviceEndpoint() {
        val preferences = InMemoryRemotePreferences()
        val repository = RemoteRepository(preferences)
        val endpointId = repository.deviceEndpointId()
        repository.savePairing(ticket())

        repository.clearActivePc()

        assertNull(RemoteRepository(preferences).activePc())
        assertEquals(endpointId, RemoteRepository(preferences).deviceEndpointId())
    }

    @Test
    fun activePcDropsLegacyRecordWithoutBridgeUrl() {
        val preferences = InMemoryRemotePreferences()
        preferences.edit {
            putString("active.endpointId", "pc-endpoint")
            putString("active.displayName", "Desk")
            putString("active.pairingUri", "lilia-remote://pair?v=1")
            putInt("active.protocolVersion", 1)
            putLong("active.lastActiveAt", 1710000000000)
        }

        assertNull(RemoteRepository(preferences).activePc())
    }

    @Test
    fun markActivePcSeenUpdatesCurrentPcLastActiveAt() {
        val preferences = InMemoryRemotePreferences()
        val repository = RemoteRepository(preferences)
        val saved = repository.savePairing(ticket())

        val updated = repository.markActivePcSeen(saved, now = 1710000001234)

        assertEquals(1710000001234, updated.lastActiveAt)
        assertEquals(1710000001234, RemoteRepository(preferences).activePc()?.lastActiveAt)
    }

    @Test
    fun markActivePcSeenIgnoresStalePcEndpoint() {
        val preferences = InMemoryRemotePreferences()
        val repository = RemoteRepository(preferences)
        repository.savePairing(ticket())
        val stalePc = RemoteRepository(preferences).activePc()!!.copy(endpointId = "pc-other")

        val result = repository.markActivePcSeen(stalePc, now = 1710000001234)

        assertEquals(stalePc, result)
        assertEquals("pc-endpoint", RemoteRepository(preferences).activePc()?.endpointId)
        assertTrue((RemoteRepository(preferences).activePc()?.lastActiveAt ?: 0L) != 1710000001234L)
    }

    @Test
    fun markActivePcSeenIgnoresStaleBridgeUrl() {
        val preferences = InMemoryRemotePreferences()
        val repository = RemoteRepository(preferences)
        repository.savePairing(ticket())
        val stalePc = RemoteRepository(preferences).activePc()!!
            .copy(bridgeUrl = "http://192.168.1.99:41478")

        val result = repository.markActivePcSeen(stalePc, now = 1710000001234)

        assertEquals(stalePc, result)
        assertEquals("http://192.168.1.12:41478", RemoteRepository(preferences).activePc()?.bridgeUrl)
        assertTrue((RemoteRepository(preferences).activePc()?.lastActiveAt ?: 0L) != 1710000001234L)
    }

    private fun ticket(bridgeUrl: String = "http://192.168.1.12:41478"): RemotePairingTicket =
        RemotePairingTicket(
            protocolVersion = 1,
            ticketId = "ticket-1",
            challenge = "challenge-1",
            endpointId = "pc-endpoint",
            pcName = "Desk",
            bridgeUrl = bridgeUrl,
            rawUri = "lilia-remote://pair?v=1",
        )
}

private class InMemoryRemotePreferences : RemotePreferences {
    private val values = mutableMapOf<String, Any?>()

    override fun getString(key: String, defaultValue: String?): String? =
        values[key] as? String ?: defaultValue

    override fun getInt(key: String, defaultValue: Int): Int =
        values[key] as? Int ?: defaultValue

    override fun getLong(key: String, defaultValue: Long): Long =
        values[key] as? Long ?: defaultValue

    override fun edit(block: RemotePreferencesEditor.() -> Unit) {
        InMemoryRemotePreferencesEditor(values).block()
    }
}

private class InMemoryRemotePreferencesEditor(
    private val values: MutableMap<String, Any?>,
) : RemotePreferencesEditor {
    override fun putString(key: String, value: String?) {
        if (value == null) {
            values.remove(key)
        } else {
            values[key] = value
        }
    }

    override fun putInt(key: String, value: Int) {
        values[key] = value
    }

    override fun putLong(key: String, value: Long) {
        values[key] = value
    }

    override fun remove(key: String) {
        values.remove(key)
    }
}
