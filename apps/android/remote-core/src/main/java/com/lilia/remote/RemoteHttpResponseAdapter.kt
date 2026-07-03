package com.lilia.remote

import org.json.JSONException
import org.json.JSONObject

object RemoteHttpResponseAdapter {
    fun parseJson(statusCode: Int, text: String): JSONObject {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) {
            error("远控桥接返回空响应（HTTP $statusCode）。")
        }
        return try {
            JSONObject(trimmed)
        } catch (_: JSONException) {
            error("远控桥接返回了非 JSON 响应（HTTP $statusCode）：${preview(trimmed)}")
        }
    }

    private fun preview(value: String): String {
        val compact = value.replace(Regex("\\s+"), " ").trim()
        return if (compact.length <= 160) compact else compact.take(157) + "…"
    }
}
