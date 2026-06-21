package com.lilia.remote

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteEnvelopeAdapterTest {
    @Test
    fun requestEnvelopeUsesTrustedDeviceEndpointAndProtocolVersion() {
        val envelope = RemoteEnvelopeAdapter.requestEnvelope(
            pc = savedPc(),
            deviceEndpointId = "android-device-1",
            request = JSONObject().put("type", "tasks.list").put("limit", 80),
            sentAt = 1710000000000,
            requestId = "android-request-1",
        )

        assertEquals("android-request-1", envelope.getString("id"))
        assertEquals(1, envelope.getInt("protocolVersion"))
        assertEquals(1710000000000, envelope.getLong("sentAt"))
        assertEquals("android-device-1", envelope.getString("deviceId"))
        assertEquals("tasks.list", envelope.getJSONObject("request").getString("type"))
        assertEquals(80, envelope.getJSONObject("request").getInt("limit"))
    }

    @Test
    fun payloadOrThrowReturnsPayloadForSuccessfulResponse() {
        val payload = RemoteEnvelopeAdapter.payloadOrThrow(
            JSONObject()
                .put("ok", true)
                .put("payload", JSONObject().put("type", "tasks.list")),
        )

        assertEquals("tasks.list", payload.getString("type"))
    }

    @Test
    fun payloadOrThrowReadsStructuredErrorMessage() {
        val err = runCatching {
            RemoteEnvelopeAdapter.payloadOrThrow(
                JSONObject()
                    .put("ok", false)
                    .put(
                        "error",
                        JSONObject()
                            .put("code", "unauthorized")
                            .put("message", "unknown remote device"),
                    ),
            )
        }.exceptionOrNull()

        assertEquals("unknown remote device", err?.message)
        assertTrue(err is RemoteBridgeException)
        assertEquals("unauthorized", (err as RemoteBridgeException).code)
    }

    @Test
    fun payloadOrThrowReadsStringErrorMessage() {
        val err = runCatching {
            RemoteEnvelopeAdapter.payloadOrThrow(
                JSONObject()
                    .put("ok", false)
                    .put("error", "pairing ticket expired"),
            )
        }.exceptionOrNull()

        assertEquals("pairing ticket expired", err?.message)
    }

    @Test
    fun throwIfErrorPreservesStructuredErrorCode() {
        val err = runCatching {
            RemoteEnvelopeAdapter.throwIfError(
                JSONObject()
                    .put("ok", false)
                    .put(
                        "error",
                        JSONObject()
                            .put("code", "invalidRequest")
                            .put("message", "pairing ticket expired")
                            .put("retryable", false),
                    ),
                fallback = "Pairing failed",
            )
        }.exceptionOrNull()

        assertTrue(err is RemoteBridgeException)
        assertEquals("invalidRequest", (err as RemoteBridgeException).code)
        assertEquals("pairing ticket expired", err.message)
    }

    private fun savedPc(): SavedPc = SavedPc(
        endpointId = "pc-endpoint",
        displayName = "Lilia PC",
        protocolVersion = 1,
        pairingUri = "lilia-remote://pair?v=1",
        bridgeUrl = "http://192.168.1.12:41478",
        lastActiveAt = 1710000000000,
    )
}
