package com.lilia.remote

import org.json.JSONObject
import java.util.UUID

class RemoteBridgeException(
    val code: String,
    override val message: String,
    val retryable: Boolean = false,
) : IllegalStateException(message)

object RemoteEnvelopeAdapter {
    fun requestEnvelope(
        pc: SavedPc,
        deviceEndpointId: String,
        request: JSONObject,
        sentAt: Long = System.currentTimeMillis(),
        requestId: String = "android-${UUID.randomUUID()}",
    ): JSONObject = JSONObject()
        .put("id", requestId)
        .put("protocolVersion", pc.protocolVersion)
        .put("sentAt", sentAt)
        .put("deviceId", deviceEndpointId)
        .put("request", request)

    fun payloadOrThrow(response: JSONObject): JSONObject {
        if (!response.optBoolean("ok")) {
            throw remoteError(response, "远控请求失败")
        }
        return response.getJSONObject("payload")
    }

    fun throwIfError(response: JSONObject, fallback: String = "远控请求失败") {
        if (!response.optBoolean("ok")) {
            throw remoteError(response, fallback)
        }
    }

    private fun errorMessage(response: JSONObject, fallback: String = "远控请求失败"): String {
        val error = response.opt("error")
        if (error is JSONObject) {
            return error.optString("message", fallback)
        }
        if (error is String && error.isNotBlank()) return error
        return fallback
    }

    private fun remoteError(response: JSONObject, fallback: String): RemoteBridgeException {
        val error = response.opt("error")
        if (error is JSONObject) {
            val message = error.optString("message", fallback)
            return RemoteBridgeException(
                code = error.optString("code", "internal"),
                message = message,
                retryable = error.optBoolean("retryable", false),
            )
        }
        return RemoteBridgeException(
            code = "internal",
            message = errorMessage(response, fallback),
        )
    }
}
