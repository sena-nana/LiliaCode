package com.lilia.remote

import android.content.Context
import android.content.SharedPreferences
import java.util.UUID

interface RemoteDeviceStore {
    fun deviceEndpointId(): String
    fun savePairing(ticket: RemotePairingTicket): SavedPc
    fun markActivePcSeen(pc: SavedPc, now: Long = System.currentTimeMillis()): SavedPc
}

class RemoteRepository internal constructor(
    private val prefs: RemotePreferences,
) : RemoteDeviceStore {
    constructor(context: Context) : this(
        AndroidRemotePreferences(context.getSharedPreferences("lilia_remote", Context.MODE_PRIVATE)),
    )

    override fun deviceEndpointId(): String {
        val existing = prefs.getString("device.endpointId", null)
        if (!existing.isNullOrBlank()) return existing
        val created = "android-${UUID.randomUUID()}"
        prefs.edit {
            putString("device.endpointId", created)
        }
        return created
    }

    fun activePc(): SavedPc? {
        val endpointId = prefs.getString("active.endpointId", null) ?: return null
        val displayName = prefs.getString("active.displayName", null) ?: "Lilia PC"
        val pairingUri = prefs.getString("active.pairingUri", null) ?: ""
        val bridgeUrl = prefs.getString("active.bridgeUrl", null)?.takeIf { it.isNotBlank() }
            ?: run {
                clearActivePc()
                return null
            }
        val protocolVersion = prefs.getInt("active.protocolVersion", 1)
        val lastActiveAt = prefs.getLong("active.lastActiveAt", 0L)
        return SavedPc(
            endpointId = endpointId,
            displayName = displayName,
            protocolVersion = protocolVersion,
            pairingUri = pairingUri,
            bridgeUrl = bridgeUrl,
            lastActiveAt = lastActiveAt,
        )
    }

    override fun savePairing(ticket: RemotePairingTicket): SavedPc {
        val pc = SavedPc(
            endpointId = ticket.endpointId,
            displayName = ticket.pcName,
            protocolVersion = ticket.protocolVersion,
            pairingUri = ticket.rawUri,
            bridgeUrl = ticket.bridgeUrl,
            lastActiveAt = System.currentTimeMillis(),
        )
        prefs.edit {
            putString("active.endpointId", pc.endpointId)
            putString("active.displayName", pc.displayName)
            putString("active.pairingUri", pc.pairingUri)
            putString("active.bridgeUrl", pc.bridgeUrl)
            putInt("active.protocolVersion", pc.protocolVersion)
            putLong("active.lastActiveAt", pc.lastActiveAt)
        }
        return pc
    }

    override fun markActivePcSeen(pc: SavedPc, now: Long): SavedPc {
        val current = activePc()
        if (current == null || !activePcConnectionMatches(current, pc)) return pc
        val updated = current.copy(lastActiveAt = now)
        prefs.edit {
            putLong("active.lastActiveAt", updated.lastActiveAt)
        }
        return updated
    }

    fun clearActivePc() {
        prefs.edit {
            remove("active.endpointId")
            remove("active.displayName")
            remove("active.pairingUri")
            remove("active.bridgeUrl")
            remove("active.protocolVersion")
            remove("active.lastActiveAt")
        }
    }
}

internal fun activePcConnectionMatches(current: SavedPc, candidate: SavedPc): Boolean =
    current.endpointId == candidate.endpointId && current.bridgeUrl == candidate.bridgeUrl

internal interface RemotePreferences {
    fun getString(key: String, defaultValue: String?): String?
    fun getInt(key: String, defaultValue: Int): Int
    fun getLong(key: String, defaultValue: Long): Long
    fun edit(block: RemotePreferencesEditor.() -> Unit)
}

internal interface RemotePreferencesEditor {
    fun putString(key: String, value: String?)
    fun putInt(key: String, value: Int)
    fun putLong(key: String, value: Long)
    fun remove(key: String)
}

private class AndroidRemotePreferences(
    private val prefs: SharedPreferences,
) : RemotePreferences {
    override fun getString(key: String, defaultValue: String?): String? =
        prefs.getString(key, defaultValue)

    override fun getInt(key: String, defaultValue: Int): Int =
        prefs.getInt(key, defaultValue)

    override fun getLong(key: String, defaultValue: Long): Long =
        prefs.getLong(key, defaultValue)

    override fun edit(block: RemotePreferencesEditor.() -> Unit) {
        val editor = prefs.edit()
        AndroidRemotePreferencesEditor(editor).block()
        editor.apply()
    }
}

private class AndroidRemotePreferencesEditor(
    private val editor: SharedPreferences.Editor,
) : RemotePreferencesEditor {
    override fun putString(key: String, value: String?) {
        if (value == null) {
            editor.remove(key)
        } else {
            editor.putString(key, value)
        }
    }

    override fun putInt(key: String, value: Int) {
        editor.putInt(key, value)
    }

    override fun putLong(key: String, value: Long) {
        editor.putLong(key, value)
    }

    override fun remove(key: String) {
        editor.remove(key)
    }
}
