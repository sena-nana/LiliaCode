package com.lilia.remote

import org.json.JSONException
import org.json.JSONObject

object RemoteHttpResponseAdapter {
    fun parseJson(statusCode: Int, text: String): JSONObject {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) {
            error("Remote bridge returned an empty response (HTTP $statusCode).")
        }
        return try {
            JSONObject(trimmed)
        } catch (_: JSONException) {
            error("Remote bridge returned a non-JSON response (HTTP $statusCode): ${preview(trimmed)}")
        }
    }

    private fun preview(value: String): String {
        val compact = value.replace(Regex("\\s+"), " ").trim()
        return if (compact.length <= 160) compact else compact.take(157) + "..."
    }
}
