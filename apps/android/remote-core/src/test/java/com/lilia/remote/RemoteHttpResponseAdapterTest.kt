package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Test

class RemoteHttpResponseAdapterTest {
    @Test
    fun parsesJsonResponse() {
        val response = RemoteHttpResponseAdapter.parseJson(200, """{ "ok": true }""")

        assertEquals(true, response.getBoolean("ok"))
    }

    @Test
    fun rejectsEmptyResponseWithStatusCode() {
        val err = runCatching {
            RemoteHttpResponseAdapter.parseJson(502, "   ")
        }.exceptionOrNull()

        assertEquals("远控桥接返回空响应（HTTP 502）。", err?.message)
    }

    @Test
    fun rejectsNonJsonResponseWithCompactPreview() {
        val err = runCatching {
            RemoteHttpResponseAdapter.parseJson(404, "<html>\n  <body>Not Found</body>\n</html>")
        }.exceptionOrNull()

        assertEquals(
            "远控桥接返回了非 JSON 响应（HTTP 404）：<html> <body>Not Found</body> </html>",
            err?.message,
        )
    }
}
